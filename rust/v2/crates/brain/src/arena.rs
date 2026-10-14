// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Variant Competition Arena — generate multiple response candidates, score them,
//! and return the winner.
//!
//! Off by default; enable via `Arena::enable()` or `lu_configure`.
//!
//! Scoring formula: score = coherence × novelty × parsimony
//!   coherence  = KU type diversity (ratio of distinct KU kinds to total KUs)
//!   novelty    = 1 - overlap with recent responses (Jaccard similarity)
//!   parsimony  = 1 - (ku_count / max_ku) — fewer KUs is better if complete

use serde::{Deserialize, Serialize};

// ── ResponseVariant ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseVariant {
    /// The generated response text.
    pub response: String,
    /// Labels of KUs used to produce this variant.
    pub ku_labels: Vec<String>,
    /// KU kind tags (e.g. "definition", "mechanism") — used for coherence scoring.
    pub ku_kinds: Vec<String>,
    /// Fraction of distinct KU kinds among all KUs used.
    pub coherence: f32,
    /// 1 - max_jaccard_with_recent_responses.
    pub novelty: f32,
    /// 1 - (ku_count / MAX_KU_BUDGET).
    pub parsimony: f32,
    /// Final combined score.
    pub score: f32,
}

impl ResponseVariant {
    pub fn new(response: impl Into<String>, ku_labels: Vec<String>, ku_kinds: Vec<String>) -> Self {
        ResponseVariant {
            response: response.into(),
            ku_labels,
            ku_kinds,
            coherence: 0.0,
            novelty: 0.0,
            parsimony: 0.0,
            score: 0.0,
        }
    }
}

// ── Arena ─────────────────────────────────────────────────────────────────────

/// Generates, scores, and selects response variants.
#[derive(Debug)]
pub struct Arena {
    /// When false, the arena is bypassed and the single default response is returned.
    pub enabled: bool,
    /// How many variants to generate per query (3–5 recommended).
    pub variant_count: usize,
    /// Ring buffer of recent winning response texts (for novelty scoring).
    recent_responses: Vec<String>,
    pub recent_window: usize,
    /// Variants that lost the competition, kept for learning.
    pub losers_log: Vec<ResponseVariant>,
    pub losers_log_max: usize,
    pub total_competitions: u64,
}

impl Default for Arena {
    fn default() -> Self {
        Arena {
            enabled: false,
            variant_count: 3,
            recent_responses: Vec::new(),
            recent_window: 20,
            losers_log: Vec::new(),
            losers_log_max: 100,
            total_competitions: 0,
        }
    }
}

impl Arena {
    pub fn new() -> Self {
        Arena::default()
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Score a single variant given the current recent-response history.
    pub fn score_variant(&self, variant: &mut ResponseVariant) {
        const MAX_KU_BUDGET: f32 = 10.0;

        // Coherence: ratio of distinct KU kinds to total KU count.
        let total_ku = variant.ku_kinds.len().max(1) as f32;
        let distinct_kinds: std::collections::HashSet<&str> =
            variant.ku_kinds.iter().map(|s| s.as_str()).collect();
        variant.coherence = (distinct_kinds.len() as f32 / total_ku).clamp(0.0, 1.0);

        // Novelty: 1 - max Jaccard similarity to any recent response.
        let words: std::collections::HashSet<&str> =
            variant.response.split_whitespace().collect();
        let max_overlap = self.recent_responses.iter().map(|r| {
            let other: std::collections::HashSet<&str> = r.split_whitespace().collect();
            let intersection = words.intersection(&other).count();
            let union = words.union(&other).count();
            if union == 0 { 0.0f32 } else { intersection as f32 / union as f32 }
        }).fold(0.0f32, f32::max);
        variant.novelty = (1.0 - max_overlap).clamp(0.0, 1.0);

        // Parsimony: fewer KUs is better, scaled against budget.
        variant.parsimony = (1.0 - (total_ku / MAX_KU_BUDGET)).clamp(0.0, 1.0);

        variant.score = variant.coherence * variant.novelty * variant.parsimony;
    }

    /// Run a competition among the provided variants.
    ///
    /// Returns the highest-scoring variant and logs the rest.
    /// If variants is empty, returns None.
    pub fn compete(&mut self, mut variants: Vec<ResponseVariant>) -> Option<ResponseVariant> {
        if variants.is_empty() {
            return None;
        }
        self.total_competitions += 1;

        // Score all variants.
        for v in &mut variants {
            self.score_variant(v);
        }

        // Sort descending by score.
        variants.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let winner = variants.remove(0);

        // Record winner in recent ring buffer.
        if self.recent_responses.len() >= self.recent_window {
            self.recent_responses.remove(0);
        }
        self.recent_responses.push(winner.response.clone());

        // Log losers.
        for loser in variants {
            if self.losers_log.len() >= self.losers_log_max {
                self.losers_log.remove(0);
            }
            self.losers_log.push(loser);
        }

        Some(winner)
    }
}
