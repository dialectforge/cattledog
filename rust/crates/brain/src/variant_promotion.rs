// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Variant Promotion
//!
//! Converts winning variants from the arena into IngestProposals
//! for human review. Includes full activation trace, scoring
//! breakdown, and comparison against other variants.
//!
//! Non-promoted variants are archived to session memory.
//! Rejected promoted variants create scars.
//!
//! Created by John Dean Martin

use serde::{Serialize, Deserialize};
use crate::variants::{VariantCluster, VariantStatus, VariantSummary};
use crate::arena::VariantArena;

// ━━━ Promotion Report ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What the promotion process produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotionReport {
    pub query: String,
    pub generation: u32,
    pub promoted: Vec<PromotedVariant>,
    pub archived: Vec<VariantSummary>,
    pub discarded_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotedVariant {
    pub variant: VariantSummary,
    pub top_activated_nodes: Vec<(String, f32)>,  // (seed_hex, strength)
    pub explanation: String,
}

/// Run the full promotion pipeline on an arena.
/// Returns a report of what was promoted vs archived vs discarded.
pub fn run_promotion(arena: &mut VariantArena, max_promote: usize) -> PromotionReport {
    // Promote top N
    let _promoted_ids = arena.promote_top(max_promote);

    // Build promoted details
    let promoted: Vec<PromotedVariant> = arena.variants
        .iter()
        .filter(|v| v.status == VariantStatus::Promoted)
        .map(|v| {
            let top_nodes: Vec<(String, f32)> = v.activated_nodes
                .iter()
                .take(10)
                .cloned()
                .collect();

            let explanation = format!(
                "Variant '{}' ({}) scored {:.3} (coherence={:.2}, novelty={:.2}, parsimony={:.2}). \
                 Activated {} nodes, top {} shown.",
                v.id,
                match &v.strategy {
                    crate::variants::PerturbationStrategy::Baseline => "baseline",
                    crate::variants::PerturbationStrategy::DomainShift { .. } => "domain-shift",
                    crate::variants::PerturbationStrategy::DepthRotation { .. } => "depth-rotation",
                    crate::variants::PerturbationStrategy::Contrarian { .. } => "contrarian",
                    crate::variants::PerturbationStrategy::CrossDomain { .. } => "cross-domain",
                },
                v.combined_score,
                v.coherence_score,
                v.novelty_score,
                v.parsimony_score,
                v.activated_nodes.len(),
                top_nodes.len(),
            );

            PromotedVariant {
                variant: VariantSummary::from(v),
                top_activated_nodes: top_nodes,
                explanation,
            }
        })
        .collect();

    // Archive the rest
    arena.archive_rest();

    let archived: Vec<VariantSummary> = arena.variants
        .iter()
        .filter(|v| v.status == VariantStatus::Archived)
        .map(VariantSummary::from)
        .collect();

    let discarded_count = arena.variants
        .iter()
        .filter(|v| v.status == VariantStatus::Discarded)
        .count();

    PromotionReport {
        query: arena.query.clone(),
        generation: arena.generation,
        promoted,
        archived,
        discarded_count,
    }
}
