// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Variant Spawner & Seed Perturbation
//!
//! Fusion 360-style hypothesis generation. Given a goal seed,
//! spawns 3-5 VariantCluster meta-node groups, each exploring
//! a different region of the knowledge graph via deterministic
//! seed perturbation strategies.
//!
//! Variants are temporary — only promoted winners enter the
//! propose→approve flow. All through DialectForge terminal bond.
//!
//! Created by John Dean Martin

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("var_{:08x}", (nanos & 0xFFFFFFFF) as u32)
}

// ━━━ Variant Status ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VariantStatus {
    /// Just created, not yet activated
    Spawned,
    /// Undergoing spreading activation
    Activating,
    /// Scored after activation
    Scored,
    /// Promoted to human review
    Promoted,
    /// Saved for future reference
    Archived,
    /// Discarded (low score)
    Discarded,
}

// ━━━ Seed Perturbation ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Strategy for how this variant's seed was derived.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PerturbationStrategy {
    /// V0: Original seed, baseline exploration
    Baseline,
    /// V1: XOR with a domain hub seed — shifts perspective
    DomainShift { hub_hex: String },
    /// V2: Bit-rotated by activation depth — deeper exploration
    DepthRotation { rotation_bits: u32 },
    /// V3: Mixed with a scar-adjacent seed — contrarian angle
    Contrarian { scar_seed_hex: String },
    /// V4: Cross-domain hub intersection — interdisciplinary
    CrossDomain { hub_a_hex: String, hub_b_hex: String },
}

/// Apply seed perturbation to get a variant's activation seed.
/// Returns a Vec<u8> that can be used to derive a pattern.
pub fn perturb_seed(base_pattern: &[f32], strategy: &PerturbationStrategy) -> Vec<f32> {
    let mut result = base_pattern.to_vec();
    match strategy {
        PerturbationStrategy::Baseline => {
            // No change — baseline variant
        }
        PerturbationStrategy::DomainShift { .. } => {
            // Shift pattern by rotating elements ~25%
            let shift = result.len() / 4;
            result.rotate_left(shift);
        }
        PerturbationStrategy::DepthRotation { rotation_bits } => {
            // Reverse chunks of the pattern for deeper exploration
            let chunk_size = (*rotation_bits as usize).max(1).min(result.len() / 2);
            for chunk in result.chunks_mut(chunk_size) {
                chunk.reverse();
            }
        }
        PerturbationStrategy::Contrarian { .. } => {
            // Invert the pattern — explore opposite semantic space
            for v in &mut result {
                *v = 1.0 - *v;
            }
            // Re-normalize
            let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut result {
                    *v /= norm;
                }
            }
        }
        PerturbationStrategy::CrossDomain { .. } => {
            // Blend first half with reversed second half
            let mid = result.len() / 2;
            let second_half: Vec<f32> = result[mid..].iter().rev().cloned().collect();
            for (i, v) in second_half.iter().enumerate() {
                if i < mid {
                    result[i] = (result[i] + v) / 2.0;
                }
            }
        }
    }
    result
}

// ━━━ Variant Cluster ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single hypothesis variant: a perturbed view of the query
/// that explores a different region of the knowledge graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariantCluster {
    pub id: String,
    pub strategy: PerturbationStrategy,
    pub status: VariantStatus,
    pub perturbed_pattern: Vec<f32>,
    /// Nodes activated by this variant's spread
    pub activated_nodes: Vec<(String, f32)>,  // (seed_hex, strength)
    /// Scoring
    pub coherence_score: f32,
    pub novelty_score: f32,
    pub parsimony_score: f32,
    pub combined_score: f32,
    pub created_at: DateTime<Utc>,
}

impl VariantCluster {
    pub fn new(strategy: PerturbationStrategy, base_pattern: &[f32]) -> Self {
        let perturbed = perturb_seed(base_pattern, &strategy);
        Self {
            id: short_id(),
            strategy,
            status: VariantStatus::Spawned,
            perturbed_pattern: perturbed,
            activated_nodes: Vec::new(),
            coherence_score: 0.0,
            novelty_score: 0.0,
            parsimony_score: 0.0,
            combined_score: 0.0,
            created_at: Utc::now(),
        }
    }

    /// Compute combined score from components.
    pub fn compute_score(&mut self) {
        self.combined_score =
            self.coherence_score * 0.6 +
            self.novelty_score * 0.3 +
            self.parsimony_score * 0.1;
    }
}

// ━━━ Variant Spawner ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maximum variants per generation (hard cap).
const MAX_VARIANTS: usize = 10;

/// Spawns variant clusters for a given base pattern.
/// Uses deterministic perturbation strategies so results are reproducible.
pub struct VariantSpawner;

impl VariantSpawner {
    /// Spawn variants for a query pattern.
    /// `hub_hexes`: known domain hub seeds (for domain-shift variants).
    /// `scar_hexes`: seeds near scars (for contrarian variants).
    /// `count`: how many variants (3-5 recommended, capped at MAX_VARIANTS).
    pub fn spawn(
        base_pattern: &[f32],
        hub_hexes: &[String],
        scar_hexes: &[String],
        count: usize,
    ) -> Vec<VariantCluster> {
        let count = count.min(MAX_VARIANTS).max(1);
        let mut variants = Vec::with_capacity(count);

        // V0: Baseline — always present
        variants.push(VariantCluster::new(
            PerturbationStrategy::Baseline,
            base_pattern,
        ));

        if count >= 2 {
            // V1: Domain shift
            let hub = hub_hexes.first()
                .cloned()
                .unwrap_or_else(|| "0000".to_string());
            variants.push(VariantCluster::new(
                PerturbationStrategy::DomainShift { hub_hex: hub },
                base_pattern,
            ));
        }

        if count >= 3 {
            // V2: Depth rotation
            variants.push(VariantCluster::new(
                PerturbationStrategy::DepthRotation { rotation_bits: 8 },
                base_pattern,
            ));
        }

        if count >= 4 {
            // V3: Contrarian
            let scar = scar_hexes.first()
                .cloned()
                .unwrap_or_else(|| "ffff".to_string());
            variants.push(VariantCluster::new(
                PerturbationStrategy::Contrarian { scar_seed_hex: scar },
                base_pattern,
            ));
        }

        if count >= 5 {
            // V4: Cross-domain
            let hub_a = hub_hexes.first()
                .cloned()
                .unwrap_or_else(|| "0000".to_string());
            let hub_b = hub_hexes.get(1)
                .cloned()
                .unwrap_or_else(|| "ffff".to_string());
            variants.push(VariantCluster::new(
                PerturbationStrategy::CrossDomain {
                    hub_a_hex: hub_a,
                    hub_b_hex: hub_b,
                },
                base_pattern,
            ));
        }

        variants
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariantSummary {
    pub id: String,
    pub strategy: String,
    pub status: String,
    pub coherence: f32,
    pub novelty: f32,
    pub parsimony: f32,
    pub combined: f32,
    pub activated_count: usize,
}

impl From<&VariantCluster> for VariantSummary {
    fn from(v: &VariantCluster) -> Self {
        let strat = match &v.strategy {
            PerturbationStrategy::Baseline => "baseline".to_string(),
            PerturbationStrategy::DomainShift { hub_hex } =>
                format!("domain_shift({})", &hub_hex[..hub_hex.len().min(8)]),
            PerturbationStrategy::DepthRotation { rotation_bits } =>
                format!("depth_rotation({})", rotation_bits),
            PerturbationStrategy::Contrarian { scar_seed_hex } =>
                format!("contrarian({})", &scar_seed_hex[..scar_seed_hex.len().min(8)]),
            PerturbationStrategy::CrossDomain { hub_a_hex, hub_b_hex } =>
                format!("cross_domain({}×{})",
                    &hub_a_hex[..hub_a_hex.len().min(6)],
                    &hub_b_hex[..hub_b_hex.len().min(6)]),
        };
        VariantSummary {
            id: v.id.clone(),
            strategy: strat,
            status: format!("{:?}", v.status),
            coherence: v.coherence_score,
            novelty: v.novelty_score,
            parsimony: v.parsimony_score,
            combined: v.combined_score,
            activated_count: v.activated_nodes.len(),
        }
    }
}
