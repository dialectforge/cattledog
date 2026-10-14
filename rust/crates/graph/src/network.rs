// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Associative Network
//!
//! The complete knowledge graph. Nodes + Bonds + traversal.
//! Every node is DF-gated. Every bond is DF-negotiated.
//! Recall works through spreading activation.

use std::collections::HashMap;
use df_core::{Seed, SeedGated};
use df_core::bond::BondProtocol;
use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::node::{KnowledgeNode, Scar, Confidence};

/// The associative memory network.
/// This IS the knowledge base. There is no separate database.
#[derive(Clone, Serialize, Deserialize)]
pub struct AssociativeNetwork {
    /// All knowledge nodes, keyed by seed hex
    nodes: HashMap<String, KnowledgeNode>,
    /// All bonds between nodes, keyed by bond seed hex
    bonds: HashMap<String, BondProtocol>,
    /// Index: node seed -> list of bond seed hexes connected to it
    bond_index: HashMap<String, Vec<String>>,
    /// All revocation scars
    scars: Vec<Scar>,
}

impl AssociativeNetwork {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            bonds: HashMap::new(),
            bond_index: HashMap::new(),
            scars: Vec::new(),
        }
    }

    // ─── Learning ───────────────────────────────────────

    /// Add a knowledge node to the network.
    /// Returns the node's seed.
    pub fn add_node(&mut self, node: KnowledgeNode) -> Seed {
        let seed = node.seed().clone();
        let key = seed.to_hex();
        self.nodes.insert(key, node);
        seed
    }

    /// Create a bond between two nodes.
    /// Returns the bond's seed, or a scar warning string.
    pub fn add_bond(
        &mut self,
        source: &Seed,
        target: &Seed,
        label: String,
        weight: f32,
        created_by: &str,
    ) -> Result<Seed, String> {
        let src_key = source.to_hex();
        let tgt_key = target.to_hex();

        // Verify both nodes exist and are active
        let src_active = self.nodes.get(&src_key)
            .map(|n| n.is_active()).unwrap_or(false);
        let tgt_active = self.nodes.get(&tgt_key)
            .map(|n| n.is_active()).unwrap_or(false);

        if !src_active || !tgt_active {
            return Err("One or both nodes are inactive or missing".into());
        }

        // Check scars
        if let Some(warning) = self.check_scars(source, target) {
            return Err(format!("SCAR_WARNING: {}", warning));
        }

        // Check for existing bond
        if let Some(existing) = self.find_bond(source, target) {
            let bond = self.bonds.get_mut(&existing.to_hex()).unwrap();
            bond.strengthen(0.1);
            return Ok(existing);
        }

        // Create the bond through DF
        let bond = BondProtocol::create(source, target, label, weight, created_by);
        let bond_seed = bond.seed.clone();
        let bond_key = bond_seed.to_hex();

        self.bond_index.entry(src_key.clone())
            .or_default().push(bond_key.clone());
        self.bond_index.entry(tgt_key.clone())
            .or_default().push(bond_key.clone());
        self.bonds.insert(bond_key, bond);

        Ok(bond_seed)
    }

    // ─── Recall (Spreading Activation) ──────────────────

    /// Recall knowledge through spreading activation.
    /// Given a set of initial node seeds with activation strengths,
    /// spread activation through bonds for `depth` hops.
    ///
    /// v2 cascade improvements:
    /// - Only spreads NEW gains per hop (not accumulated totals)
    /// - Per-node activation ceiling prevents runaway accumulation
    /// - Diminishing returns on convergent paths (first path matters most)
    pub fn spread_activation(
        &mut self,
        initial: &[(Seed, f32)],
        depth: usize,
        top_k: usize,
    ) -> RecallResult {
        // Per-node activation ceiling.
        const ACTIVATION_CEILING: f32 = 200.0;

        // Hard cap on nodes fully processed.  This is the key scalability
        // guarantee: regardless of graph size or depth, we visit at most
        // max_visits nodes.  top_k * 50 keeps a generous search frontier
        // (e.g., depth-3 from 10 seeds, avg 16 bonds = ~2560 reachable) while
        // staying fast at 500K+ nodes.  We always process the HIGHEST-activation
        // nodes first (priority queue), so top_k quality is preserved.
        let max_visits = (top_k * 50).max(500).min(5000);

        let mut activations: HashMap<String, f32> = HashMap::new();
        let mut traversal_path: Vec<TraversalHop> = Vec::new();
        let scar_warnings: Vec<String> = Vec::new();

        // Priority queue: highest gain explored first.
        // Each entry is (activation_as_ordered_bits, hop, key_string).
        // We encode f32 as u32 bits for Ord — valid because all activations
        // are positive finite floats, where bit order == value order.
        use std::collections::BinaryHeap;
        #[derive(Eq, PartialEq)]
        struct PQEntry { gain_bits: u32, hop: usize, key: String }
        impl Ord for PQEntry {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.gain_bits.cmp(&other.gain_bits)
                    .then_with(|| other.hop.cmp(&self.hop)) // shallower hops first on tie
            }
        }
        impl PartialOrd for PQEntry {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
        }

        let mut heap: BinaryHeap<PQEntry> = BinaryHeap::new();

        // Phase 1: Seed the heap with initial activations.
        for (seed, strength) in initial {
            let key = seed.to_hex();
            if let Some(node) = self.nodes.get_mut(&key) {
                if node.is_revoked() { continue; }
                node.activate();
                let capped = (*strength).min(ACTIVATION_CEILING);
                activations.insert(key.clone(), capped);
                heap.push(PQEntry { gain_bits: capped.to_bits(), hop: 0, key });
            }
        }

        // Phase 2: Best-first spreading activation with node cap.
        //
        // We pop the entry with the highest gain and spread from it.
        // A visited set prevents re-processing the same node twice.
        // The 0.7^hop decay + 0.1 gain threshold ensure low-quality
        // peripheral nodes never make it into the heap in the first place.
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut visits: usize = 0;

        while let Some(PQEntry { gain_bits: _, hop, key }) = heap.pop() {
            if !visited.insert(key.clone()) { continue; } // already processed
            if visits >= max_visits { break; }
            visits += 1;

            if hop >= depth { continue; }

            let spread_strength = *activations.get(&key).unwrap_or(&0.0);
            let decay = 0.7f32.powi(hop as i32 + 1);

            // Frequency-aware hub capping for hops 0 and 1.
            // High-degree "hub" nodes (common words like "loop", "function",
            // "variable") can have thousands of bonds and flood the activation
            // set with loosely related nodes on the first two expansions.
            // When a node's fan-out exceeds HUB_THRESHOLD on an early hop,
            // we only propagate to the top HUB_HOP_CAP strongest bonds.
            // After hop 1 the activation set is already focused, so normal
            // expansion resumes.
            const HUB_THRESHOLD: usize = 500;
            const HUB_HOP_CAP: usize = 75;

            let raw_bond_keys: Vec<String> = self.bond_index
                .get(&key)
                .cloned()
                .unwrap_or_default();

            let effective_bond_keys: Vec<String> = if hop < 2 && raw_bond_keys.len() > HUB_THRESHOLD {
                // Collect active bonds with their weights, sort strongest-first,
                // keep only the top HUB_HOP_CAP neighbours.
                let mut weighted: Vec<(String, f32)> = raw_bond_keys.iter()
                    .filter_map(|bk| {
                        self.bonds.get(bk)
                            .filter(|b| b.active)
                            .map(|b| (bk.clone(), b.weight))
                    })
                    .collect();
                weighted.sort_by(|a, b| b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal));
                weighted.truncate(HUB_HOP_CAP);
                weighted.into_iter().map(|(bk, _)| bk).collect()
            } else {
                raw_bond_keys
            };

            for bond_key in &effective_bond_keys {
                // Determine neighbor key without holding mutable bond ref.
                let (neighbor_key, bw) = {
                    let bond = match self.bonds.get(bond_key) {
                        Some(b) if b.active => b,
                        _ => continue,
                    };
                    let nkey = if bond.source.to_hex() == key {
                        bond.target.to_hex()
                    } else if bond.bidirectional {
                        bond.source.to_hex()
                    } else {
                        continue;
                    };
                    (nkey, bond.weight)
                };

                if matches!(self.nodes.get(&neighbor_key), Some(n) if n.is_revoked()) {
                    continue;
                }

                let spread = spread_strength * bw * decay;
                let existing = *activations.get(&neighbor_key).unwrap_or(&0.0);
                let diminished = spread * (50.0 / (50.0 + existing));

                if diminished > 0.1 {
                    let new_total = (existing + diminished).min(ACTIVATION_CEILING);
                    let actual_gain = new_total - existing;
                    if actual_gain > 0.0 {
                        *activations.entry(neighbor_key.clone()).or_insert(0.0) += actual_gain;
                        // Clamp after update
                        if let Some(v) = activations.get_mut(&neighbor_key) {
                            *v = v.min(ACTIVATION_CEILING);
                        }

                        // Hebbian: strengthen the traversed bond.
                        if let Some(bond) = self.bonds.get_mut(bond_key) {
                            bond.strengthen(0.05);
                        }

                        traversal_path.push(TraversalHop {
                            from: key.clone(),
                            bond: bond_key.clone(),
                            to: neighbor_key.clone(),
                            strength: actual_gain,
                            hop: hop as u32,
                        });

                        if !visited.contains(&neighbor_key) {
                            if let Some(node) = self.nodes.get_mut(&neighbor_key) {
                                node.activate();
                            }
                            heap.push(PQEntry {
                                gain_bits: actual_gain.to_bits(),
                                hop: hop + 1,
                                key: neighbor_key,
                            });
                        }
                    }
                }
            }
        }

        // Phase 3: Compile top_k results.
        let mut sorted: Vec<(String, f32)> = activations.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(top_k);

        let total: f32 = sorted.iter().map(|(_, v)| v).sum();
        let total = if total > 0.0 { total } else { 1.0 };

        let activated_nodes: Vec<ActivatedNode> = sorted.iter()
            .filter_map(|(key, strength)| {
                self.nodes.get(key).map(|node| ActivatedNode {
                    seed_hex: key.clone(),
                    label: node.label.clone(),
                    strength: *strength,
                    contribution: strength / total,
                    confidence: node.confidence.clone(),
                })
            })
            .collect();

        let seed_report: HashMap<String, f32> = activated_nodes.iter()
            .map(|n| (n.seed_hex.clone(), n.contribution))
            .collect();

        RecallResult {
            activated_nodes,
            traversal_path,
            seed_report,
            scar_warnings,
        }
    }

    // ─── Revocation ─────────────────────────────────────

    /// Revoke a specific seed. Node or bond.
    /// Leaves a scar. Traces forward to children.
    pub fn revoke(&mut self, seed: &Seed, reason: &str) -> RevocationReport {
        let key = seed.to_hex();
        let mut report = RevocationReport {
            revoked_seed: key.clone(),
            reason: reason.to_string(),
            entity_type: None,
            label: None,
            scar_created: false,
            children_to_review: Vec::new(),
        };

        // Try revoking as node
        if let Some(node) = self.nodes.get_mut(&key) {
            node.revoke(reason);
            report.entity_type = Some("node".into());
            report.label = Some(node.label.clone());

            // Create scar
            let scar = Scar {
                revoked_seed: seed.clone(),
                label: node.label.clone(),
                reason: reason.to_string(),
                revoked_at: Utc::now(),
                pattern_fingerprint: node.pattern.clone(),
            };
            self.scars.push(scar);
            report.scar_created = true;

            // Deactivate connected bonds
            if let Some(bond_keys) = self.bond_index.get(&key) {
                for bk in bond_keys.clone() {
                    if let Some(bond) = self.bonds.get_mut(&bk) {
                        bond.active = false;
                    }
                }
            }

            // Find children that derive from this node
            for (k, n) in &self.nodes {
                if n.parent_seeds.iter().any(|p| p.to_hex() == key) && n.is_active() {
                    report.children_to_review.push((k.clone(), n.label.clone()));
                }
            }

            return report;
        }

        // Try revoking as bond
        if let Some(bond) = self.bonds.get_mut(&key) {
            bond.active = false;
            report.entity_type = Some("bond".into());
            report.label = Some(bond.label.clone());

            // Create scar from midpoint of connected node patterns
            let src = self.nodes.get(&bond.source.to_hex());
            let tgt = self.nodes.get(&bond.target.to_hex());
            if let (Some(s), Some(t)) = (src, tgt) {
                let midpoint: Vec<f32> = s.pattern.iter()
                    .zip(t.pattern.iter())
                    .map(|(a, b)| (a + b) / 2.0)
                    .collect();
                let norm: f32 = midpoint.iter().map(|x| x * x).sum::<f32>().sqrt();
                let normalized = if norm > 0.0 {
                    midpoint.iter().map(|x| x / norm).collect()
                } else {
                    midpoint
                };

                let scar = Scar {
                    revoked_seed: seed.clone(),
                    label: format!("bond: [{}] <-> [{}]", s.label, t.label),
                    reason: reason.to_string(),
                    revoked_at: Utc::now(),
                    pattern_fingerprint: normalized,
                };
                self.scars.push(scar);
                report.scar_created = true;
            }
        }

        report
    }

    // ─── Temporary Node Management (Thinking Layer) ────

    /// Deactivate a node WITHOUT creating a scar.
    /// Used for cleaning up temporary thinking nodes (goals, sub-goals,
    /// hypothesis bonds) that were never promoted to permanent.
    /// Unlike revoke(), this leaves no trace — the node simply stops
    /// existing. Connected bonds are also deactivated silently.
    /// Returns true if the node was found and deactivated.
    pub fn deactivate_temporary(&mut self, seed: &Seed) -> bool {
        let key = seed.to_hex();

        // Try as node
        if let Some(node) = self.nodes.get_mut(&key) {
            if !node.is_active() {
                return false; // Already inactive
            }
            // Deactivate without creating a scar
            node.revoke("temporary_cleanup"); // sets active=false
            // Note: revoke() on KnowledgeNode just sets active=false
            // and stores the reason. No scar is created here because
            // we don't push to self.scars.

            // Silently deactivate connected bonds
            if let Some(bond_keys) = self.bond_index.get(&key) {
                for bk in bond_keys.clone() {
                    if let Some(bond) = self.bonds.get_mut(&bk) {
                        bond.active = false;
                    }
                }
            }
            return true;
        }

        // Try as bond
        if let Some(bond) = self.bonds.get_mut(&key) {
            bond.active = false;
            return true;
        }

        false
    }

    // ─── Self-Modification (v0 7.6) ────────────────────

    /// Strengthen the bond between two nodes by seed hex.
    /// Returns the new weight, or error if no bond exists.
    pub fn strengthen_bond(&mut self, src_hex: &str, tgt_hex: &str, amount: f32) -> Result<f32, String> {
        let bond_key = self.find_bond_hex(src_hex, tgt_hex)
            .ok_or("No bond exists between these nodes")?;
        let bond = self.bonds.get_mut(&bond_key)
            .ok_or("Bond not found")?;
        bond.strengthen(amount);
        Ok(bond.weight)
    }

    /// Weaken the bond between two nodes by seed hex.
    /// Returns the new weight, or error if no bond exists.
    pub fn weaken_bond(&mut self, src_hex: &str, tgt_hex: &str, amount: f32) -> Result<f32, String> {
        let bond_key = self.find_bond_hex(src_hex, tgt_hex)
            .ok_or("No bond exists between these nodes")?;
        let bond = self.bonds.get_mut(&bond_key)
            .ok_or("Bond not found")?;
        bond.weaken(amount);
        Ok(bond.weight)
    }

    /// Apply organic decay to ALL bonds. Bonds that haven't been
    /// activated recently lose weight. Bonds that have decayed to
    /// zero are marked dead. This is organic forgetting — the brain
    /// forgets what it doesn't use.
    ///
    /// Returns (bonds_decayed, bonds_killed) counts.
    pub fn decay_all_bonds(&mut self, amount: f32) -> (usize, usize) {
        let mut decayed = 0usize;
        let mut killed = 0usize;

        for bond in self.bonds.values_mut() {
            if !bond.active { continue; }

            // Only decay bonds that haven't been used recently.
            // "Recently" = activated at least once since last decay.
            // Bonds used during recall get strengthened (0.05 in spread_activation),
            // so active bonds naturally resist decay.
            bond.weaken(amount);
            decayed += 1;

            if bond.is_dead() {
                bond.active = false;
                killed += 1;
            }
        }

        (decayed, killed)
    }

    /// Prune dead bonds from the network entirely.
    /// Returns count of bonds removed.
    pub fn prune_dead_bonds(&mut self) -> usize {
        let dead_keys: Vec<String> = self.bonds.iter()
            .filter(|(_, b)| !b.active || b.is_dead())
            .map(|(k, _)| k.clone())
            .collect();

        let count = dead_keys.len();

        for key in &dead_keys {
            self.bonds.remove(key);
            // Clean up bond_index references
            for idx in self.bond_index.values_mut() {
                idx.retain(|k| k != key);
            }
        }

        count
    }

    /// Get bond weight between two nodes (by hex). Returns None if no bond.
    pub fn bond_weight(&self, src_hex: &str, tgt_hex: &str) -> Option<f32> {
        let bond_key = self.find_bond_hex(src_hex, tgt_hex)?;
        self.bonds.get(&bond_key).map(|b| b.weight)
    }

    /// Find bond key between two nodes by hex strings.
    fn find_bond_hex(&self, src_hex: &str, tgt_hex: &str) -> Option<String> {
        let bond_keys = self.bond_index.get(src_hex)?;
        for bk in bond_keys {
            if let Some(bond) = self.bonds.get(bk) {
                if bond.active
                    && (bond.target.to_hex() == tgt_hex || bond.source.to_hex() == tgt_hex)
                {
                    return Some(bk.clone());
                }
            }
        }
        None
    }

    /// Get all bonds for a node (by hex), with their targets and weights.
    /// Used by self_reflect to see what's connected.
    pub fn node_bonds(&self, node_hex: &str) -> Vec<(String, String, f32)> {
        let bond_keys = match self.bond_index.get(node_hex) {
            Some(keys) => keys,
            None => return Vec::new(),
        };

        let mut result = Vec::new();
        for bk in bond_keys {
            if let Some(bond) = self.bonds.get(bk) {
                if !bond.active { continue; }
                let neighbor = if bond.source.to_hex() == node_hex {
                    bond.target.to_hex()
                } else {
                    bond.source.to_hex()
                };
                result.push((bk.clone(), neighbor, bond.weight));
            }
        }
        result
    }

    // ─── Queries ────────────────────────────────────────

    pub fn get_node(&self, seed: &Seed) -> Option<&KnowledgeNode> {
        self.nodes.get(&seed.to_hex())
    }

    pub fn get_node_by_hex(&self, hex: &str) -> Option<&KnowledgeNode> {
        self.nodes.get(hex).filter(|n| n.is_active())
    }

    /// Get an active bond by its hex key.
    pub fn get_bond_by_hex(&self, hex: &str) -> Option<&BondProtocol> {
        self.bonds.get(hex).filter(|b| b.active)
    }

    /// Get active bond count for a single node (by hex key).
    pub fn bond_count_for(&self, node_hex: &str) -> Option<usize> {
        self.bond_index.get(node_hex).map(|keys| {
            keys.iter()
                .filter(|bk| self.bonds.get(*bk).map_or(false, |b| b.active))
                .count()
        })
    }

    /// Mutable access to a single node by seed.
    /// Used by approve methods to set metadata after creation.
    pub fn get_node_mut(&mut self, seed: &Seed) -> Option<&mut KnowledgeNode> {
        self.nodes.get_mut(&seed.to_hex())
    }

    /// Iterate all active nodes with their seed hex keys.
    /// Used by brain for pattern similarity searches.
    pub fn active_nodes(&self) -> impl Iterator<Item = (&str, &KnowledgeNode)> {
        self.nodes.iter()
            .filter(|(_, n)| n.is_active())
            .map(|(k, n)| (k.as_str(), n))
    }

    /// Mutable access to all active nodes (for migration/resize operations).
    pub fn active_nodes_mut(&mut self) -> impl Iterator<Item = (&str, &mut KnowledgeNode)> {
        self.nodes.iter_mut()
            .filter(|(_, n)| n.is_active())
            .map(|(k, n)| (k.as_str(), n))
    }

    /// Get a map of node hex -> label for all active nodes.
    /// Used by the reasoning engine to build readable chains.
    pub fn node_label_map(&self) -> HashMap<String, String> {
        self.nodes.iter()
            .filter(|(_, n)| n.is_active())
            .map(|(k, n)| (k.clone(), n.label.clone()))
            .collect()
    }

    /// Get a map of node hex -> confidence string for all active nodes.
    /// Used by the visualizer to show trust level.
    pub fn node_confidence_map(&self) -> HashMap<String, String> {
        self.nodes.iter()
            .filter(|(_, n)| n.is_active())
            .map(|(k, n)| {
                let conf_str = match &n.confidence {
                    crate::node::Confidence::Terminal => "Terminal",
                    crate::node::Confidence::Verified => "Verified",
                    crate::node::Confidence::Unverified => "Unverified",
                    crate::node::Confidence::Contested => "Contested",
                };
                (k.clone(), conf_str.to_string())
            })
            .collect()
    }

    /// Get a map of bond hex -> label for all active bonds.
    /// Used by the reasoning engine to describe connections.
    pub fn bond_label_map(&self) -> HashMap<String, String> {
        self.bonds.iter()
            .filter(|(_, b)| b.active)
            .map(|(k, b)| (k.clone(), b.label.clone()))
            .collect()
    }

    /// Get all active bonds as (source_hex, target_hex, weight) tuples.
    /// Used by the visualizer to render the graph.
    pub fn viz_bonds(&self) -> Vec<(String, String, f32)> {
        self.bonds.values()
            .filter(|b| b.active)
            .map(|b| (b.source.to_hex(), b.target.to_hex(), b.weight))
            .collect()
    }

    /// Get bond count per node for all active nodes.
    /// Used by the visualizer for node sizing.
    pub fn bond_counts(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for (node_hex, bond_keys) in &self.bond_index {
            let active_count = bond_keys.iter()
                .filter(|bk| self.bonds.get(*bk).map_or(false, |b| b.active))
                .count();
            if active_count > 0 {
                counts.insert(node_hex.clone(), active_count);
            }
        }
        counts
    }

    pub fn scars(&self) -> &[Scar] {
        &self.scars
    }

    /// Add a scar directly (e.g. from failed thinking sub-goals).
    /// Unlike revoke(), this does not deactivate any node — it just
    /// records a warning pattern so future associations get flagged.
    pub fn add_scar(&mut self, scar: Scar) {
        self.scars.push(scar);
    }

    /// O(1) node count — use instead of stats().active_nodes when only the
    /// count is needed and revoked nodes are negligible (they stay in the map).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn stats(&self) -> NetworkStats {
        NetworkStats {
            total_nodes: self.nodes.len(),
            active_nodes: self.nodes.values().filter(|n| n.is_active()).count(),
            revoked_nodes: self.nodes.values().filter(|n| n.is_revoked()).count(),
            total_bonds: self.bonds.len(),
            active_bonds: self.bonds.values().filter(|b| b.active).count(),
            dead_bonds: self.bonds.values().filter(|b| !b.active || b.is_dead()).count(),
            scars: self.scars.len(),
        }
    }

    // ─── Internal ───────────────────────────────────────

    fn check_scars(&self, source: &Seed, target: &Seed) -> Option<String> {
        let src = self.nodes.get(&source.to_hex())?;
        let tgt = self.nodes.get(&target.to_hex())?;

        // Content-based scar check: compare the actual text of the nodes
        // being bonded against the scar's label text. Pattern-based checking
        // is disabled because the pattern engine produces vectors too tightly
        // clustered to discriminate (all dot products > 0.97).
        //
        // A scar blocks a bond only if BOTH nodes share significant word
        // overlap with the scarred content. This prevents "the sun is a rock"
        // from blocking bonds between unrelated nodes while still catching
        // attempts to re-associate the specific revoked concepts.
        let src_words: std::collections::HashSet<String> = src.label
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect();
        let tgt_words: std::collections::HashSet<String> = tgt.label
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect();

        for scar in &self.scars {
            let scar_words: std::collections::HashSet<String> = scar.label
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() >= 3)
                .map(|w| w.to_string())
                .collect();

            if scar_words.is_empty() { continue; }

            // Both nodes must share >50% of the scar's words
            let src_overlap = src_words.intersection(&scar_words).count() as f32
                / scar_words.len() as f32;
            let tgt_overlap = tgt_words.intersection(&scar_words).count() as f32
                / scar_words.len() as f32;

            if src_overlap > 0.5 && tgt_overlap > 0.5 {
                return Some(format!(
                    "Matches scar: {} (reason: {})",
                    scar.label, scar.reason
                ));
            }
        }

        None
    }

    fn find_bond(&self, source: &Seed, target: &Seed) -> Option<Seed> {
        let src_key = source.to_hex();
        let tgt_key = target.to_hex();

        let bond_keys = self.bond_index.get(&src_key)?;
        for bk in bond_keys {
            if let Some(bond) = self.bonds.get(bk) {
                if bond.active
                    && (bond.target.to_hex() == tgt_key || bond.source.to_hex() == tgt_key)
                {
                    return Some(bond.seed.clone());
                }
            }
        }
        None
    }

    /// Public scar check: can a bond between these two seeds exist
    /// without triggering a scar? Returns None if safe, Some(warning)
    /// if a scar would block it.
    ///
    /// Used by the thinking layer to check scar conflicts without
    /// creating (and leaking) probe bonds.
    pub fn has_scar_conflict(&self, source: &Seed, target: &Seed) -> Option<String> {
        self.check_scars(source, target)
    }
}

impl Default for AssociativeNetwork {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Result Types ───────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecallResult {
    pub activated_nodes: Vec<ActivatedNode>,
    pub traversal_path: Vec<TraversalHop>,
    pub seed_report: HashMap<String, f32>,
    pub scar_warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivatedNode {
    pub seed_hex: String,
    pub label: String,
    pub strength: f32,
    pub contribution: f32,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraversalHop {
    pub from: String,
    pub bond: String,
    pub to: String,
    pub strength: f32,
    pub hop: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevocationReport {
    pub revoked_seed: String,
    pub reason: String,
    pub entity_type: Option<String>,
    pub label: Option<String>,
    pub scar_created: bool,
    pub children_to_review: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkStats {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub revoked_nodes: usize,
    pub total_bonds: usize,
    pub active_bonds: usize,
    pub dead_bonds: usize,
    pub scars: usize,
}
