// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Rumination — background consolidation that discovers latent bonds between
//! knowledge nodes that co-appear in episodic memories but aren't yet linked.
//!
//! ## Phase 7 enhancements
//!
//! ### Contradiction detection
//! After bond discovery, scan for contradictory bonds:
//! - Node A →CAUSES→ B  **and**  Node C →OPPOSES→ B  (causal + opposing)
//! - Two Definition nodes with the same subject connected by an Opposes bond
//!
//! ### KU promotion
//! Raw KUs with strong bonds (total weight > 3.0) **and** ≥5 episode appearances
//! are candidates for re-typing via `TemporalLobe::extract()`.  Successful
//! promotions are counted in `RuminationReport::promotions`.
//!
//! ### Bond decay (Hebbian forgetting)
//! Bonds whose source **and** target nodes do not appear in any of the last 50
//! episodes are considered inactive.  Each such bond loses 0.01 weight per
//! rumination cycle.  Bonds that fall below 0.1 are flagged (not deleted).
//!
//! ## Algorithm (original)
//! 1. Build a co-activation map from episodic memories.
//! 2. Build the set of existing active bond pairs.
//! 3. For each pair whose co-activation count ≥ 2:
//!    - New bond → create a weak bond (weight 0.5) with inferred type.
//!    - Existing bond → strengthen slightly (+0.05).
//! 4. Return a `RuminationReport` summarising what happened.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use df_core::{Seed, SeedGated};
use graph::bond::{BondType, TypedBond};
use graph::knowledge_unit::{KnowledgeUnit, KuKind};
use graph::network::AssociativeNetwork;

use crate::hippocampus::EpisodicMemory;
use crate::temporal::{ExtractionContext, TemporalLobe};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default cooldown between rumination passes (30 seconds).
pub const DEFAULT_COOLDOWN_MS: u64 = 30_000;

/// Minimum co-activation count before a new bond is created.
const CO_ACTIVATION_THRESHOLD: usize = 2;

/// Weight assigned to newly-discovered bonds.
const NEW_BOND_WEIGHT: f32 = 0.5;

/// Hebbian gain applied when strengthening an existing bond.
const STRENGTHEN_GAIN: f32 = 0.05;

/// Decay amount per cycle for bonds not activated in the last 50 episodes.
const BOND_DECAY_RATE: f32 = 0.01;

/// Bonds below this weight are flagged as weak (but not deleted).
const WEAK_BOND_THRESHOLD: f32 = 0.1;

/// Total bond-weight threshold for KU promotion candidacy.
const PROMOTION_WEIGHT_THRESHOLD: f32 = 3.0;

/// Episode-appearance count threshold for KU promotion candidacy.
const PROMOTION_EPISODE_THRESHOLD: usize = 5;

/// How many recent episodes to consider for the "recently active" bond check.
const RECENT_EPISODE_WINDOW: usize = 50;

// ── ContradictionRecord ───────────────────────────────────────────────────────

/// A detected semantic contradiction between two knowledge nodes.
#[derive(Debug, Clone)]
pub struct ContradictionRecord {
    /// One of the two conflicting nodes.
    pub node_a: Seed,
    /// The other conflicting node.
    pub node_b: Seed,
    /// Human-readable description of why they conflict.
    pub description: String,
    /// When the contradiction was detected.
    pub detected_at: DateTime<Utc>,
}

impl ContradictionRecord {
    pub fn new(node_a: Seed, node_b: Seed, description: impl Into<String>) -> Self {
        ContradictionRecord {
            node_a,
            node_b,
            description: description.into(),
            detected_at: Utc::now(),
        }
    }
}

// ── RuminationReport ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct RuminationReport {
    /// Number of new weak bonds created during this pass.
    pub bonds_discovered: usize,
    /// Number of existing bonds strengthened during this pass.
    pub bonds_strengthened: usize,
    /// Number of distinct nodes visited (appeared in evaluated pairs).
    pub nodes_visited: usize,
    /// Contradictions detected during this pass.
    pub contradictions: Vec<ContradictionRecord>,
    /// Number of Raw KUs promoted to a typed KU.
    pub promotions: usize,
    /// Number of bonds that were decayed this cycle.
    pub bonds_decayed: usize,
    /// Number of bonds now flagged as weak (weight < WEAK_BOND_THRESHOLD).
    pub bonds_flagged_weak: usize,
}

// ── RuminationEngine ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct RuminationEngine {
    /// Cooldown between passes.
    pub cooldown: Duration,
    /// Wall-clock time of the most recent pass (None = never run).
    pub last_rumination: Option<Instant>,
    /// Snapshot of bond activation counts from the previous pass.
    /// Used to determine which bonds were inactive during this cycle.
    bond_activation_snapshot: HashMap<String, u64>,
}

impl Default for RuminationEngine {
    fn default() -> Self {
        RuminationEngine::new(DEFAULT_COOLDOWN_MS)
    }
}

impl RuminationEngine {
    pub fn new(cooldown_ms: u64) -> Self {
        RuminationEngine {
            cooldown: Duration::from_millis(cooldown_ms),
            last_rumination: None,
            bond_activation_snapshot: HashMap::new(),
        }
    }

    /// True if the cooldown has not yet elapsed since the last pass.
    pub fn is_cooling_down(&self) -> bool {
        match self.last_rumination {
            Some(t) => t.elapsed() < self.cooldown,
            None    => false,
        }
    }

    /// Run one rumination pass over the given network and episodic memories.
    ///
    /// Returns an empty report immediately if the cooldown has not elapsed.
    ///
    /// The `temporal` lobe is used for the KU promotion pass; pass
    /// `&brain.temporal` from `Brain::ruminate()`.
    pub fn step(
        &mut self,
        network: &mut AssociativeNetwork,
        episodes: &[EpisodicMemory],
        temporal: &mut TemporalLobe,
        enable_contradiction_detection: bool,
    ) -> RuminationReport {
        if self.is_cooling_down() {
            return RuminationReport::default();
        }

        self.last_rumination = Some(Instant::now());

        // ── 1. Build co-activation map ────────────────────────────────────────
        let mut coact: HashMap<(String, String), usize> = HashMap::new();

        for episode in episodes {
            let seeds = &episode.seeds_involved;
            for i in 0..seeds.len() {
                for j in (i + 1)..seeds.len() {
                    let a = seeds[i].to_hex();
                    let b = seeds[j].to_hex();
                    let key = canonical_pair(a, b);
                    *coact.entry(key).or_insert(0) += 1;
                }
            }
        }

        // ── 2. Build existing bond set ────────────────────────────────────────
        let existing: HashSet<(String, String)> = network
            .all_bonds()
            .iter()
            .filter(|b| b.active)
            .map(|b| canonical_pair(b.source_seed.to_hex(), b.target_seed.to_hex()))
            .collect();

        // ── 3. Snapshot KU kinds + subjects per node ──────────────────────────
        let ku_kinds: HashMap<String, KuKind> = network
            .active_nodes()
            .iter()
            .map(|n| (n.seed().to_hex(), n.knowledge_unit.kind()))
            .collect();

        let ku_subjects: HashMap<String, String> = network
            .active_nodes()
            .iter()
            .filter_map(|n| {
                n.knowledge_unit.subject().map(|s| (n.seed().to_hex(), s.to_lowercase()))
            })
            .collect();

        // ── 4. Process co-activated pairs ─────────────────────────────────────
        let mut bonds_discovered = 0usize;
        let mut bonds_strengthened = 0usize;
        let mut nodes_seen: HashSet<String> = HashSet::new();

        let qualifying: Vec<((String, String), bool)> = coact
            .iter()
            .filter(|(_, &count)| count >= CO_ACTIVATION_THRESHOLD)
            .map(|(pair, _)| {
                let is_new = !existing.contains(pair);
                (pair.clone(), is_new)
            })
            .collect();

        for ((hex_a, hex_b), is_new) in qualifying {
            nodes_seen.insert(hex_a.clone());
            nodes_seen.insert(hex_b.clone());

            if is_new {
                let kind_a = ku_kinds.get(&hex_a).copied();
                let kind_b = ku_kinds.get(&hex_b).copied();
                let bond_type = infer_bond_type(kind_a, kind_b);

                let seed_a = match Seed::from_hex(&hex_a) { Ok(s) => s, Err(_) => continue };
                let seed_b = match Seed::from_hex(&hex_b) { Ok(s) => s, Err(_) => continue };

                let bond = TypedBond::new(seed_a, seed_b, bond_type, NEW_BOND_WEIGHT);
                network.add_bond(bond);
                bonds_discovered += 1;
            } else {
                let bond_seeds: Vec<Seed> = network
                    .all_bonds()
                    .into_iter()
                    .filter(|b| {
                        b.active && {
                            let s = b.source_seed.to_hex();
                            let t = b.target_seed.to_hex();
                            (s == hex_a && t == hex_b) || (s == hex_b && t == hex_a)
                        }
                    })
                    .map(|b| b.seed.clone())
                    .collect();

                for bs in bond_seeds {
                    if let Some(bond) = network.get_bond_mut(&bs) {
                        bond.strengthen(STRENGTHEN_GAIN);
                        bonds_strengthened += 1;
                    }
                }
            }
        }

        // ── 5. Contradiction detection ────────────────────────────────────────
        let contradictions = if enable_contradiction_detection {
            detect_contradictions(network, &ku_subjects)
        } else {
            vec![]
        };

        // ── 6. KU promotion pass ──────────────────────────────────────────────
        let promotions = promote_raw_kus(network, episodes, temporal);

        // ── 7. Bond decay pass ────────────────────────────────────────────────
        let (bonds_decayed, bonds_flagged_weak) =
            decay_inactive_bonds(network, episodes, &mut self.bond_activation_snapshot);

        RuminationReport {
            bonds_discovered,
            bonds_strengthened,
            nodes_visited: nodes_seen.len(),
            contradictions,
            promotions,
            bonds_decayed,
            bonds_flagged_weak,
        }
    }
}

// ── Contradiction detection ───────────────────────────────────────────────────

/// Scan the network for two categories of contradiction:
///
/// 1. **Causal conflict**: A →CAUSES→ B and C →OPPOSES→ B (A and C conflict).
/// 2. **Definition conflict**: two Definition nodes with the same subject
///    connected by an Opposes bond.
fn detect_contradictions(
    network: &AssociativeNetwork,
    ku_subjects: &HashMap<String, String>,
) -> Vec<ContradictionRecord> {
    let mut records: Vec<ContradictionRecord> = Vec::new();
    let bonds = network.all_bonds();

    // ── a) Causal conflicts ───────────────────────────────────────────────────
    // Build: target_hex → Vec<(source_hex, bond_type)>
    let mut target_map: HashMap<String, Vec<(String, BondType)>> = HashMap::new();
    for bond in bonds.iter().filter(|b| b.active) {
        target_map
            .entry(bond.target_seed.to_hex())
            .or_default()
            .push((bond.source_seed.to_hex(), bond.bond_type));
    }

    for (target_hex, sources) in &target_map {
        let causers: Vec<&str> = sources
            .iter()
            .filter(|(_, bt)| *bt == BondType::Causes)
            .map(|(src, _)| src.as_str())
            .collect();
        let opposers: Vec<&str> = sources
            .iter()
            .filter(|(_, bt)| *bt == BondType::Opposes)
            .map(|(src, _)| src.as_str())
            .collect();

        if causers.is_empty() || opposers.is_empty() {
            continue;
        }

        for causer_hex in &causers {
            for opposer_hex in &opposers {
                if let (Ok(seed_a), Ok(seed_b)) = (
                    Seed::from_hex(causer_hex),
                    Seed::from_hex(opposer_hex),
                ) {
                    records.push(ContradictionRecord::new(
                        seed_a,
                        seed_b,
                        format!(
                            "Causal conflict: one node CAUSES '{}' while another OPPOSES it",
                            target_hex
                        ),
                    ));
                }
            }
        }
    }

    // ── b) Definition conflicts ───────────────────────────────────────────────
    // Find Opposes bonds between two Definition nodes that share a subject.
    for bond in bonds.iter().filter(|b| b.active && b.bond_type == BondType::Opposes) {
        let src_hex = bond.source_seed.to_hex();
        let tgt_hex = bond.target_seed.to_hex();

        let src_subj = ku_subjects.get(&src_hex);
        let tgt_subj = ku_subjects.get(&tgt_hex);

        if let (Some(s), Some(t)) = (src_subj, tgt_subj) {
            if s == t {
                // Same subject but opposing bond → conflict
                if let (Ok(seed_a), Ok(seed_b)) = (
                    Seed::from_hex(&src_hex),
                    Seed::from_hex(&tgt_hex),
                ) {
                    // Avoid duplicates from symmetric bonds
                    if !records.iter().any(|r| {
                        (r.node_a.to_hex() == src_hex && r.node_b.to_hex() == tgt_hex)
                        || (r.node_a.to_hex() == tgt_hex && r.node_b.to_hex() == src_hex)
                    }) {
                        records.push(ContradictionRecord::new(
                            seed_a,
                            seed_b,
                            format!("Definition conflict: two nodes share subject '{}' but oppose each other", s),
                        ));
                    }
                }
            }
        }
    }

    records
}

// ── KU promotion pass ─────────────────────────────────────────────────────────

/// Find Raw KUs that are strongly connected and frequently seen in episodes,
/// and attempt to re-type them via the temporal lobe.
///
/// Returns the number of successful promotions.
fn promote_raw_kus(
    network: &mut AssociativeNetwork,
    episodes: &[EpisodicMemory],
    temporal: &mut TemporalLobe,
) -> usize {
    // Count episode appearances per seed hex.
    let mut episode_counts: HashMap<String, usize> = HashMap::new();
    for ep in episodes {
        for seed in &ep.seeds_involved {
            *episode_counts.entry(seed.to_hex()).or_insert(0) += 1;
        }
    }

    // Sum bond weights per node (source or target).
    let mut bond_weight_sums: HashMap<String, f32> = HashMap::new();
    for bond in network.all_bonds().iter().filter(|b| b.active) {
        *bond_weight_sums.entry(bond.source_seed.to_hex()).or_insert(0.0) += bond.weight;
        *bond_weight_sums.entry(bond.target_seed.to_hex()).or_insert(0.0) += bond.weight;
    }

    // Collect candidate seeds: Raw KUs above both thresholds.
    let candidates: Vec<(Seed, String)> = network
        .active_nodes()
        .iter()
        .filter(|n| {
            if n.knowledge_unit.kind() != KuKind::Raw {
                return false;
            }
            let hex = n.seed().to_hex();
            let weight_ok = bond_weight_sums.get(&hex).copied().unwrap_or(0.0)
                > PROMOTION_WEIGHT_THRESHOLD;
            let episode_ok = episode_counts.get(&hex).copied().unwrap_or(0)
                >= PROMOTION_EPISODE_THRESHOLD;
            weight_ok && episode_ok
        })
        .map(|n| (n.seed().clone(), n.knowledge_unit.raw_text().to_string()))
        .collect();

    let mut promoted = 0usize;

    for (seed, raw_text) in candidates {
        let ctx = ExtractionContext::new(&raw_text);
        let result = temporal.extract(&ctx);

        // Look for the first non-Raw typed KU.
        if let Some(typed_ku) = result
            .units
            .into_iter()
            .find(|ku| ku.kind() != KuKind::Raw)
        {
            if let Some(node) = network.get_node_mut(&seed) {
                node.knowledge_unit = typed_ku;
                promoted += 1;
            }
        }
    }

    promoted
}

// ── Bond decay pass ───────────────────────────────────────────────────────────

/// Decay bonds whose endpoints have not appeared in the last
/// `RECENT_EPISODE_WINDOW` episodes.
///
/// Returns `(bonds_decayed, bonds_flagged_weak)`.
fn decay_inactive_bonds(
    network: &mut AssociativeNetwork,
    episodes: &[EpisodicMemory],
    snapshot: &mut HashMap<String, u64>,
) -> (usize, usize) {
    // Build the set of seeds that appear in the last N episodes.
    let window = episodes.len().saturating_sub(RECENT_EPISODE_WINDOW);
    let recent_seeds: HashSet<String> = episodes[window..]
        .iter()
        .flat_map(|ep| ep.seeds_involved.iter().map(|s| s.to_hex()))
        .collect();

    // Collect bond seeds to update (avoid borrow issues).
    let to_decay: Vec<Seed> = network
        .all_bonds()
        .iter()
        .filter(|b| {
            b.active
                && !recent_seeds.contains(&b.source_seed.to_hex())
                && !recent_seeds.contains(&b.target_seed.to_hex())
        })
        .map(|b| b.seed.clone())
        .collect();

    let mut bonds_decayed = 0usize;
    let mut bonds_flagged_weak = 0usize;

    for bond_seed in &to_decay {
        if let Some(bond) = network.get_bond_mut(bond_seed) {
            bond.weight = (bond.weight - BOND_DECAY_RATE).max(0.0);
            bonds_decayed += 1;
            if bond.weight < WEAK_BOND_THRESHOLD {
                bonds_flagged_weak += 1;
            }
        }
    }

    // Update snapshot for next pass.
    *snapshot = network
        .all_bonds()
        .iter()
        .map(|b| (b.seed.to_hex(), b.activation_count))
        .collect();

    (bonds_decayed, bonds_flagged_weak)
}

// ── Bond type inference ───────────────────────────────────────────────────────

/// Infer a `BondType` from the KU kinds of the two participating nodes.
pub fn infer_bond_type(kind_a: Option<KuKind>, kind_b: Option<KuKind>) -> BondType {
    let a = kind_a.unwrap_or(KuKind::Raw);
    let b = kind_b.unwrap_or(KuKind::Raw);

    if a == KuKind::Causal || b == KuKind::Causal {
        return BondType::Causes;
    }
    if a == KuKind::Definition && b == KuKind::Definition {
        return BondType::IsA;
    }
    if a == KuKind::Composition || b == KuKind::Composition {
        return BondType::HasPart;
    }
    if a == KuKind::Mechanism || b == KuKind::Mechanism
        || a == KuKind::Procedure || b == KuKind::Procedure
    {
        return BondType::Requires;
    }
    if a == KuKind::Example || b == KuKind::Example {
        return BondType::ExampleOf;
    }
    BondType::Context
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return a canonical (lexicographically ordered) pair of hex strings.
fn canonical_pair(a: String, b: String) -> (String, String) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Extract the KuKind from any KnowledgeUnit reference.
#[allow(dead_code)]
pub fn ku_kind(ku: &KnowledgeUnit) -> KuKind {
    ku.kind()
}
