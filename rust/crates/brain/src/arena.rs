// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Variant Arena & Scoring
//!
//! Runs parallel spreading activation for all variants from the
//! same query. Scores each on coherence × novelty × parsimony.
//! Supports multiple generations if no variant clears threshold.
//!
//! Created by John Dean Martin

use std::collections::HashSet;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use crate::variants::{VariantCluster, VariantStatus, VariantSummary};

/// Default promotion threshold: combined_score must exceed this.
const PROMOTION_THRESHOLD: f32 = 0.5;

/// Maximum generations before giving up.
const MAX_GENERATIONS: u32 = 3;

// ━━━ Arena ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The arena where variants compete.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariantArena {
    pub query: String,
    pub variants: Vec<VariantCluster>,
    pub generation: u32,
    pub promotion_threshold: f32,
    pub created_at: DateTime<Utc>,
}

impl VariantArena {
    pub fn new(query: &str, variants: Vec<VariantCluster>) -> Self {
        Self {
            query: query.to_string(),
            variants,
            generation: 1,
            promotion_threshold: PROMOTION_THRESHOLD,
            created_at: Utc::now(),
        }
    }

    /// Score all variants based on their activation results.
    /// Call this after running spread_activation for each variant.
    ///
    /// `high_confidence_hexes`: set of seed hexes with Terminal/Verified confidence.
    /// `total_active_nodes`: total node count for parsimony normalization.
    pub fn score_all(
        &mut self,
        high_confidence_hexes: &HashSet<String>,
        total_active_nodes: usize,
    ) {
        // Collect all activated node sets for novelty computation
        let all_activated: Vec<HashSet<String>> = self.variants
            .iter()
            .map(|v| v.activated_nodes.iter().map(|(h, _)| h.clone()).collect())
            .collect();

        for (i, variant) in self.variants.iter_mut().enumerate() {
            if variant.status == VariantStatus::Discarded {
                continue;
            }

            let my_nodes: HashSet<String> = variant.activated_nodes
                .iter()
                .map(|(h, _)| h.clone())
                .collect();

            // Coherence: fraction of activated nodes that are high-confidence
            let high_conf_count = my_nodes.intersection(high_confidence_hexes).count();
            variant.coherence_score = if my_nodes.is_empty() {
                0.0
            } else {
                high_conf_count as f32 / my_nodes.len() as f32
            };

            // Novelty: nodes ONLY this variant reached
            let mut unique_count = 0;
            for node in &my_nodes {
                let others_have_it = all_activated.iter()
                    .enumerate()
                    .any(|(j, s)| j != i && s.contains(node));
                if !others_have_it {
                    unique_count += 1;
                }
            }
            variant.novelty_score = if my_nodes.is_empty() {
                0.0
            } else {
                unique_count as f32 / my_nodes.len() as f32
            };

            // Parsimony: fewer activated nodes = simpler = better
            variant.parsimony_score = if total_active_nodes == 0 {
                1.0
            } else {
                1.0 - (my_nodes.len() as f32 / total_active_nodes as f32).min(1.0)
            };

            variant.compute_score();
            variant.status = VariantStatus::Scored;
        }

        // Sort by combined score descending
        self.variants.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Get variants that clear the promotion threshold.
    pub fn promotable(&self) -> Vec<&VariantCluster> {
        self.variants
            .iter()
            .filter(|v| {
                v.combined_score >= self.promotion_threshold
                    && v.status == VariantStatus::Scored
            })
            .collect()
    }

    /// Promote top N variants (mark them as Promoted).
    pub fn promote_top(&mut self, n: usize) -> Vec<String> {
        let mut promoted = Vec::new();
        for variant in self.variants.iter_mut() {
            if promoted.len() >= n { break; }
            if variant.combined_score >= self.promotion_threshold
                && variant.status == VariantStatus::Scored
            {
                variant.status = VariantStatus::Promoted;
                promoted.push(variant.id.clone());
            }
        }
        promoted
    }

    /// Archive non-promoted variants.
    pub fn archive_rest(&mut self) {
        for variant in &mut self.variants {
            if variant.status == VariantStatus::Scored {
                variant.status = VariantStatus::Archived;
            }
        }
    }

    /// Can we run another generation?
    pub fn can_retry(&self) -> bool {
        self.generation < MAX_GENERATIONS
            && self.promotable().is_empty()
    }

    /// Advance to next generation (increments counter).
    pub fn next_generation(&mut self) {
        self.generation += 1;
    }

    /// Get a variant by ID.
    pub fn get_variant(&self, id: &str) -> Option<&VariantCluster> {
        self.variants.iter().find(|v| v.id == id)
    }

    /// Get a mutable variant by ID.
    pub fn get_variant_mut(&mut self, id: &str) -> Option<&mut VariantCluster> {
        self.variants.iter_mut().find(|v| v.id == id)
    }

    /// Build status summary.
    pub fn status(&self) -> ArenaStatus {
        ArenaStatus {
            query: self.query.clone(),
            generation: self.generation,
            variant_count: self.variants.len(),
            promotion_threshold: self.promotion_threshold,
            variants: self.variants.iter().map(VariantSummary::from).collect(),
        }
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArenaStatus {
    pub query: String,
    pub generation: u32,
    pub variant_count: usize,
    pub promotion_threshold: f32,
    pub variants: Vec<VariantSummary>,
}
