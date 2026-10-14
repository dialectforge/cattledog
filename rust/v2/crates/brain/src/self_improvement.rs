// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Self-improvement module — performance tracking, meta-reflection, and
//! Phase 3 self-tuning recall parameters.
//!
//! Phase 1: RecallQualityLog — relevance scoring per query.
//! Phase 2: QueryPatternIndex, HotPathIndex, node demotion candidates.
//! Phase 3 additions:
//!   - `RecallParameters`: tunable knobs consumed by Brain::respond().
//!   - `ParameterTuner`: P-controller that nudges params every N queries
//!     based on running hit-rate and quality metrics.
//!   - `TuneAdjustment`: one record per tuning pass, kept for inspection.

use std::collections::HashMap;

use graph::bond::BondType;
use graph::network::{ActivatedNode, TraversalHop};
use serde::{Deserialize, Serialize};

// ── RecallParameters ──────────────────────────────────────────────────────────

/// Tunable knobs that control how Brain::respond() retrieves knowledge.
///
/// All fields have conservative defaults.  The `ParameterTuner` P-controller
/// adjusts these incrementally based on observed recall quality; all values
/// are clamped to safe ranges so no single adjustment can break recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallParameters {
    /// Maximum candidate nodes returned from keyword search and spread
    /// activation combined.  Higher → more context, slower ranking.
    pub top_k: usize,
    /// Minimum activation strength required to propagate a hop during spread
    /// activation.  Lower → more traversal noise; higher → sparser recall.
    pub activation_threshold: f32,
    /// Bond traversal depth for spread activation (Definition queries always
    /// use depth=1 regardless of this value).
    pub bond_traversal_depth: u32,
    /// Per-KU-kind score multipliers keyed by lowercase kind name
    /// (e.g. `"definition"`, `"raw"`).  Applied after keyword + spread.
    pub type_weighting: std::collections::HashMap<String, f32>,
}

impl RecallParameters {
    pub const TOP_K_MIN: usize = 5;
    pub const TOP_K_MAX: usize = 50;
    pub const DEPTH_MIN: u32   = 1;
    pub const DEPTH_MAX: u32   = 4;
    pub const THRESH_MIN: f32  = 0.01;
    pub const THRESH_MAX: f32  = 0.50;
}

impl Default for RecallParameters {
    fn default() -> Self {
        let mut weights = std::collections::HashMap::new();
        for kind in &[
            "definition", "mechanism", "composition", "purpose",
            "contrast", "causal", "example", "procedure", "code",
        ] {
            weights.insert(kind.to_string(), 1.0_f32);
        }
        // Raw KUs are slightly less preferred — they lack semantic structure.
        weights.insert("raw".to_string(), 0.9_f32);
        RecallParameters {
            top_k: 20,
            activation_threshold: 0.05,
            bond_traversal_depth: 2,
            type_weighting: weights,
        }
    }
}

// ── TuneAdjustment ────────────────────────────────────────────────────────────

/// One record written each time the P-controller fires.  Kept in a ring buffer
/// inside `ParameterTuner` for the `lu_tune_status` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuneAdjustment {
    /// Query counter at which the tuning pass ran.
    pub at_query: u64,
    /// Signed change applied to `top_k` (0 = no change).
    pub delta_top_k: i32,
    /// Signed change applied to `bond_traversal_depth` (0 = no change).
    pub delta_depth: i32,
    /// Signed change applied to `activation_threshold` (0.0 = no change).
    pub delta_threshold: f32,
    /// Snapshot of `recall_hit_rate` at the moment of tuning.
    pub recall_hit_rate_at_tune: f32,
    /// Snapshot of `avg_response_quality` at the moment of tuning.
    pub avg_quality_at_tune: f32,
    /// Human-readable summary of this pass.
    pub note: String,
}

// ── ParameterTuner ────────────────────────────────────────────────────────────

/// Proportional (P) controller for `RecallParameters`.
///
/// Fires every `tune_interval` queries once `PerformanceMetrics` has enough
/// samples.  Each pass makes small, clamped nudges: ±2 on `top_k`, ±1 on
/// `bond_traversal_depth`, ±0.005 on `activation_threshold`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterTuner {
    /// Fire the controller every this many queries.
    pub tune_interval: u64,
    /// Ring buffer of recent tuning records (capped at 20).
    pub adjustment_history: Vec<TuneAdjustment>,
    /// P-gain for top_k: `Δtop_k ≈ kp_top_k × hit_rate_error`.
    pub kp_top_k: f32,
    /// P-gain for depth: `Δdepth ≈ kp_depth × quality_error`.
    pub kp_depth: f32,
}

impl Default for ParameterTuner {
    fn default() -> Self {
        ParameterTuner {
            tune_interval:      10,
            adjustment_history: Vec::new(),
            kp_top_k:           5.0,  // error=0.2 → nudge top_k by 1
            kp_depth:           1.0,  // error≥1.0 → nudge depth by ±1
        }
    }
}

impl ParameterTuner {
    const TARGET_HIT_RATE: f32 = 0.70;
    const TARGET_QUALITY:  f32 = 0.60;
    const MAX_HISTORY:     usize = 20;

    /// Run the P-controller if `total_queries` is a multiple of
    /// `tune_interval` and there are enough samples.  Mutates `params` in
    /// place with clamped nudges.  Returns `true` when a pass ran.
    pub fn maybe_tune(
        &mut self,
        params: &mut RecallParameters,
        metrics: &PerformanceMetrics,
        total_queries: u64,
    ) -> bool {
        if total_queries == 0
            || total_queries % self.tune_interval != 0
            || metrics.total_queries < self.tune_interval
        {
            return false;
        }

        let hit_err     = Self::TARGET_HIT_RATE - metrics.recall_hit_rate;
        let quality_err = Self::TARGET_QUALITY  - metrics.avg_response_quality;

        let mut adj = TuneAdjustment {
            at_query:               total_queries,
            recall_hit_rate_at_tune: metrics.recall_hit_rate,
            avg_quality_at_tune:     metrics.avg_response_quality,
            ..Default::default()
        };

        // ── top_k: driven by recall hit-rate error ────────────────────────────
        // Nudge at most ±2 per pass for conservatism.
        let dk = ((self.kp_top_k * hit_err) as i32).clamp(-2, 2);
        if dk != 0 {
            let new_k = ((params.top_k as i32 + dk)
                .clamp(RecallParameters::TOP_K_MIN as i32,
                       RecallParameters::TOP_K_MAX as i32)) as usize;
            if new_k != params.top_k {
                adj.delta_top_k = new_k as i32 - params.top_k as i32;
                params.top_k = new_k;
            }
        }

        // ── bond depth: driven by quality error ───────────────────────────────
        // ±1 per pass; deeper traversal surfaces more context.
        let dd = ((self.kp_depth * quality_err) as i32).clamp(-1, 1);
        if dd != 0 {
            let new_d = ((params.bond_traversal_depth as i32 + dd)
                .clamp(RecallParameters::DEPTH_MIN as i32,
                       RecallParameters::DEPTH_MAX as i32)) as u32;
            if new_d != params.bond_traversal_depth {
                adj.delta_depth = new_d as i32 - params.bond_traversal_depth as i32;
                params.bond_traversal_depth = new_d;
            }
        }

        // ── activation threshold: minor nudge driven by hit-rate error ────────
        // Lower threshold → more nodes pass the activation filter.
        if hit_err > 0.1 {
            let new_t = (params.activation_threshold - 0.005)
                .clamp(RecallParameters::THRESH_MIN, RecallParameters::THRESH_MAX);
            adj.delta_threshold = new_t - params.activation_threshold;
            params.activation_threshold = new_t;
        } else if hit_err < -0.1 {
            let new_t = (params.activation_threshold + 0.005)
                .clamp(RecallParameters::THRESH_MIN, RecallParameters::THRESH_MAX);
            adj.delta_threshold = new_t - params.activation_threshold;
            params.activation_threshold = new_t;
        }

        adj.note = format!(
            "hit={:.0}% qual={:.0}% → Δk={:+} Δd={:+} Δθ={:+.4}",
            metrics.recall_hit_rate       * 100.0,
            metrics.avg_response_quality  * 100.0,
            adj.delta_top_k,
            adj.delta_depth,
            adj.delta_threshold,
        );

        self.adjustment_history.push(adj);
        if self.adjustment_history.len() > Self::MAX_HISTORY {
            self.adjustment_history.remove(0);
        }
        true
    }
}

// ── PerformanceMetrics ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Running average: (KUs recalled / query) normalised to 0–1.
    pub avg_response_quality: f32,
    /// Fraction of queries that returned ≥1 KU.
    pub recall_hit_rate: f32,
    /// Fraction of consolidation passes that strengthened ≥1 bond.
    pub consolidation_effectiveness: f32,
    pub total_queries: u64,
    pub total_hits: u64,
    pub total_consolidations: u64,
    pub consolidation_hits: u64,
    /// Topics seen in episodes with consistently 0-hit recall.
    pub weak_topics: Vec<String>,
}

impl PerformanceMetrics {
    const EMA_ALPHA: f32 = 0.1;

    pub fn record_query(&mut self, ku_count: usize) {
        self.total_queries += 1;
        let hit = ku_count > 0;
        if hit {
            self.total_hits += 1;
        }
        self.recall_hit_rate = self.total_hits as f32 / self.total_queries as f32;

        // Quality = normalised KU count (cap at 10 for perfect score).
        let quality = (ku_count as f32 / 10.0).clamp(0.0, 1.0);
        self.avg_response_quality = Self::EMA_ALPHA * quality
            + (1.0 - Self::EMA_ALPHA) * self.avg_response_quality;
    }

    pub fn record_consolidation(&mut self, bonds_strengthened: usize) {
        self.total_consolidations += 1;
        if bonds_strengthened > 0 {
            self.consolidation_hits += 1;
        }
        self.consolidation_effectiveness =
            self.consolidation_hits as f32 / self.total_consolidations.max(1) as f32;
    }

    /// Overall health score (0–1).
    pub fn health_score(&self) -> f32 {
        if self.total_queries == 0 {
            return 0.5;
        }
        (self.avg_response_quality + self.recall_hit_rate + self.consolidation_effectiveness) / 3.0
    }
}

// ── Phase 1: RecallQualityRecord ──────────────────────────────────────────────

/// One query's recall quality snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallQualityRecord {
    /// Truncated to 120 chars to avoid storing the full query.
    pub query_preview: String,
    /// Relevance fraction 0–1. See scoring algorithm in compute_relevance().
    pub relevance_score: f32,
    /// Number of activated nodes returned.
    pub node_count: usize,
    /// Average strength of top-5 nodes.
    pub avg_strength: f32,
    /// RFC 3339 timestamp.
    pub recorded_at: String,
}

// ── Phase 1: RecallQualityLog ─────────────────────────────────────────────────

/// Rolling window of recent recall quality records.
/// Capped at `max_records` to prevent unbounded growth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallQualityLog {
    pub records: Vec<RecallQualityRecord>,
    pub max_records: usize,
}

impl Default for RecallQualityLog {
    fn default() -> Self {
        RecallQualityLog {
            records: Vec::new(),
            max_records: 500,
        }
    }
}

impl RecallQualityLog {
    /// Push a record, evicting the oldest if the window is full.
    pub fn push(&mut self, r: RecallQualityRecord) {
        if self.records.len() >= self.max_records {
            self.records.remove(0);
        }
        self.records.push(r);
    }

    /// Simple mean of the last `n` records' relevance scores.
    /// Returns 0.0 if the log is empty.
    pub fn recent_avg(&self, n: usize) -> f32 {
        let slice: Vec<_> = self.records.iter().rev().take(n).collect();
        if slice.is_empty() {
            return 0.0;
        }
        slice.iter().map(|r| r.relevance_score).sum::<f32>() / slice.len() as f32
    }

    /// The `n` records with the lowest relevance scores.
    pub fn worst_n(&self, n: usize) -> Vec<&RecallQualityRecord> {
        let mut sorted: Vec<&RecallQualityRecord> = self.records.iter().collect();
        sorted.sort_by(|a, b| {
            a.relevance_score
                .partial_cmp(&b.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }

    /// The `n` records with the highest relevance scores.
    pub fn best_n(&self, n: usize) -> Vec<&RecallQualityRecord> {
        let mut sorted: Vec<&RecallQualityRecord> = self.records.iter().collect();
        sorted.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }
}

// ── Phase 1: Relevance scoring ────────────────────────────────────────────────

/// Score how relevant the recalled nodes are to the query.
///
/// Algorithm: fraction of query content-words (len ≥ 4, not stop-words) that
/// appear in at least one of the top-5 recalled nodes' raw text.  Per-node
/// scores are averaged.  Returns 0.5 if no content words can be extracted
/// (query too short / all stop-words), and 0.0 if no nodes were recalled.
fn compute_relevance(query: &str, nodes: &[ActivatedNode]) -> f32 {
    const STOP: &[&str] = &[
        "used", "that", "with", "from", "have", "this", "what",
        "does", "work", "about", "tell", "make", "more", "many",
    ];

    let content_words: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| w.len() >= 4 && !STOP.contains(&w.as_str()))
        .collect();

    if content_words.is_empty() {
        return 0.5; // unknown — assume neutral
    }

    let top5: Vec<_> = nodes.iter().take(5).collect();
    if top5.is_empty() {
        return 0.0;
    }

    let node_scores: Vec<f32> = top5
        .iter()
        .map(|n| {
            let raw = n.knowledge_unit.raw_text().to_lowercase();
            let matched = content_words
                .iter()
                .filter(|w| raw.contains(w.as_str()))
                .count();
            matched as f32 / content_words.len() as f32
        })
        .collect();

    node_scores.iter().sum::<f32>() / node_scores.len() as f32
}

// ── Phase 2: Pattern key extraction ──────────────────────────────────────────

/// Extract a stable pattern key from a query: sorted bag of content words.
///
/// Uses the same stop-word list as `compute_relevance`.  Returns an empty
/// string if no content words remain (caller should skip recording).
fn extract_pattern_key(query: &str) -> String {
    const STOP: &[&str] = &[
        "used", "that", "with", "from", "have", "this", "what",
        "does", "work", "about", "tell", "make", "more", "many",
    ];

    let mut words: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| w.len() >= 4 && !STOP.contains(&w.as_str()))
        .collect();

    words.sort();
    words.dedup();
    words.join(" ")
}

// ── Phase 2: QueryPatternIndex ────────────────────────────────────────────────

/// Rolling quality score for a recurring query pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternEntry {
    /// Sorted, joined content words — e.g. "activation graph spread".
    pub pattern_key: String,
    /// EMA quality score (α = 0.2).
    pub ema_quality: f32,
    /// Number of times this pattern has been seen.
    pub hit_count: u32,
}

/// Index of query patterns and their quality history.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct QueryPatternIndex {
    pub entries: HashMap<String, PatternEntry>,
}

impl QueryPatternIndex {
    const EMA_ALPHA: f32 = 0.2;

    pub fn record(&mut self, pattern_key: &str, quality: f32) {
        let entry = self.entries
            .entry(pattern_key.to_string())
            .or_insert_with(|| PatternEntry {
                pattern_key: pattern_key.to_string(),
                ema_quality: quality,
                hit_count: 0,
            });
        entry.ema_quality =
            Self::EMA_ALPHA * quality + (1.0 - Self::EMA_ALPHA) * entry.ema_quality;
        entry.hit_count += 1;
    }

    /// The `n` patterns with the lowest EMA quality (that have been seen ≥ `min_hits` times).
    pub fn worst_patterns(&self, n: usize, min_hits: u32) -> Vec<&PatternEntry> {
        let mut sorted: Vec<&PatternEntry> = self
            .entries
            .values()
            .filter(|e| e.hit_count >= min_hits)
            .collect();
        sorted.sort_by(|a, b| {
            a.ema_quality
                .partial_cmp(&b.ema_quality)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }

    /// The `n` patterns with the highest EMA quality.
    pub fn best_patterns(&self, n: usize) -> Vec<&PatternEntry> {
        let mut sorted: Vec<&PatternEntry> = self.entries.values().collect();
        sorted.sort_by(|a, b| {
            b.ema_quality
                .partial_cmp(&a.ema_quality)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }
}

// ── Phase 2: HotPathIndex ─────────────────────────────────────────────────────

/// A recalled traversal chain that led to a high-quality result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    /// Bond types traversed in order — e.g. [IsA, Causes, HasPart].  Max 3 hops.
    pub bond_chain: Vec<BondType>,
    /// Quality score the last time this chain was recorded.
    pub quality: f32,
    /// How many times this exact chain has been seen in high-quality recalls.
    pub count: u32,
}

/// Index of bond-type chains that consistently led to high-quality recalls.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HotPathIndex {
    /// Key = bond types joined with "→", e.g. "IsA→Causes".
    pub paths: HashMap<String, HotPath>,
}

impl HotPathIndex {
    pub fn record(&mut self, chain: &[BondType], quality: f32) {
        if chain.is_empty() {
            return;
        }
        let key = chain
            .iter()
            .map(|b| format!("{:?}", b))
            .collect::<Vec<_>>()
            .join("→");
        let entry = self.paths.entry(key).or_insert_with(|| HotPath {
            bond_chain: chain.to_vec(),
            quality,
            count: 0,
        });
        entry.quality = quality;
        entry.count += 1;
    }

    /// Top `n` chains sorted by quality × count (composite usefulness score).
    pub fn top_n(&self, n: usize) -> Vec<&HotPath> {
        let mut sorted: Vec<&HotPath> = self.paths.values().collect();
        sorted.sort_by(|a, b| {
            let sa = a.quality * a.count as f32;
            let sb = b.quality * b.count as f32;
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }
}

// ── SelfImprovement ───────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct SelfImprovement {
    pub metrics: PerformanceMetrics,
    /// Topics consistently returning 0 KUs — identified during analysis.
    pub weak_coverage_topics: Vec<String>,
    pub analysis_count: u64,

    // ── Phase 1 ───────────────────────────────────────────────────────────────
    /// Rolling window of per-query recall quality records.
    pub quality_log: RecallQualityLog,

    // ── Phase 2 ───────────────────────────────────────────────────────────────
    /// Quality history per query-word-bag pattern.
    pub pattern_index: QueryPatternIndex,
    /// Bond-type chains that lead to high-quality recalls.
    pub hot_path_index: HotPathIndex,
    /// Per-node recall quality history: seed hex → (low_count, high_count).
    /// Low = relevance < 0.2, High = relevance > 0.6.
    pub node_quality_history: HashMap<String, (u32, u32)>,

    // ── Phase 3 ───────────────────────────────────────────────────────────────
    /// Tunable recall parameters consumed by Brain::respond().
    pub params: RecallParameters,
    /// P-controller that adjusts `params` every N queries.
    pub tuner: ParameterTuner,
}

impl SelfImprovement {
    pub fn new() -> Self {
        SelfImprovement::default()
    }

    /// Record the result of a query (called from Brain::respond).
    ///
    /// Computes a keyword-based relevance score, appends a quality record,
    /// updates the query-pattern index, records hot traversal paths, and
    /// accumulates per-node quality history for demotion tracking.
    pub fn record_query(
        &mut self,
        query: &str,
        recalled_nodes: &[ActivatedNode],
        traversal_path: &[TraversalHop],
    ) {
        let ku_count = recalled_nodes.len();
        self.metrics.record_query(ku_count);

        let relevance = compute_relevance(query, recalled_nodes);

        let top5_count = recalled_nodes.len().min(5).max(1) as f32;
        let avg_strength = recalled_nodes
            .iter()
            .take(5)
            .map(|n| n.strength)
            .sum::<f32>()
            / top5_count;

        // Phase 1: quality log
        self.quality_log.push(RecallQualityRecord {
            query_preview: query.chars().take(120).collect(),
            relevance_score: relevance,
            node_count: ku_count,
            avg_strength,
            recorded_at: chrono::Utc::now().to_rfc3339(),
        });

        // Phase 2a: query pattern index
        let pattern_key = extract_pattern_key(query);
        if !pattern_key.is_empty() {
            self.pattern_index.record(&pattern_key, relevance);
        }

        // Phase 2b: hot path index — only record for high-quality recalls
        if relevance > 0.6 && !traversal_path.is_empty() {
            let top5_seeds: std::collections::HashSet<String> = recalled_nodes
                .iter()
                .take(5)
                .map(|n| n.seed.to_hex())
                .collect();

            // Single-hop chains from hops that arrived at a top-5 node.
            let relevant_hops: Vec<&TraversalHop> = traversal_path
                .iter()
                .filter(|h| top5_seeds.contains(&h.to.to_hex()))
                .collect();

            for hop in &relevant_hops {
                self.hot_path_index.record(&[hop.bond_type], relevance);
            }

            // 2-hop chains: consecutive hops where the first ends where the second begins.
            let n = relevant_hops.len();
            for i in 0..n.saturating_sub(1) {
                let h1 = relevant_hops[i];
                for h2 in relevant_hops.iter().skip(i + 1) {
                    if h2.from.to_hex() == h1.to.to_hex() && h2.hop == h1.hop + 1 {
                        self.hot_path_index.record(&[h1.bond_type, h2.bond_type], relevance);
                        break;
                    }
                }
            }
        }

        // Phase 2c: per-node quality history
        for node in recalled_nodes.iter().take(5) {
            let hex = node.seed.to_hex();
            let entry = self.node_quality_history.entry(hex).or_insert((0, 0));
            if relevance < 0.2 {
                entry.0 += 1; // low-quality appearance
            } else if relevance > 0.6 {
                entry.1 += 1; // high-quality appearance
            }
        }
    }

    /// Record a consolidation pass result.
    pub fn record_consolidation(&mut self, bonds_strengthened: usize) {
        self.metrics.record_consolidation(bonds_strengthened);
    }

    /// Identify node seeds that appeared in ≥5 low-quality recalls (relevance < 0.2)
    /// and never in a high-quality recall (relevance > 0.6).
    /// Returns seed hexes to consider demoting from Unverified → Contested.
    pub fn demotion_candidates(&self) -> Vec<String> {
        self.node_quality_history
            .iter()
            .filter(|(_, (low, high))| *low >= 5 && *high == 0)
            .map(|(seed_hex, _)| seed_hex.clone())
            .collect()
    }

    /// Analyse node coverage and return improvement suggestions as strings.
    ///
    /// Suggestions are appropriate for queuing as WonderItems.
    pub fn analyze(&mut self, node_labels: &[(String, String)]) -> Vec<String> {
        self.analysis_count += 1;
        let mut suggestions: Vec<String> = Vec::new();

        // Find KU kind imbalances.
        let mut kind_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (_, kind) in node_labels {
            *kind_counts.entry(kind.as_str()).or_insert(0) += 1;
        }

        let total = node_labels.len().max(1);
        let raw_pct = kind_counts.get("raw").copied().unwrap_or(0) as f32 / total as f32;
        if raw_pct > 0.5 {
            suggestions.push(
                "More than half my knowledge is unstructured. \
                 I should focus on learning well-defined concepts."
                    .to_string(),
            );
        }

        let def_count = kind_counts.get("definition").copied().unwrap_or(0);
        let mech_count = kind_counts.get("mechanism").copied().unwrap_or(0);
        if def_count > 0 && mech_count == 0 {
            suggestions.push(
                "I have definitions but no mechanisms. \
                 I should learn *how* things work, not just what they are."
                    .to_string(),
            );
        }

        // Weak recall: hit rate below 50%.
        if self.metrics.total_queries >= 5 && self.metrics.recall_hit_rate < 0.5 {
            suggestions.push(format!(
                "My recall hit rate is only {:.0}%. \
                 I should ingest more knowledge on topics I'm frequently asked about.",
                self.metrics.recall_hit_rate * 100.0
            ));
        }

        // Low consolidation effectiveness.
        if self.metrics.total_consolidations >= 3
            && self.metrics.consolidation_effectiveness < 0.3
        {
            suggestions.push(
                "Consolidation rarely strengthens bonds — my episodes may cover isolated topics. \
                 Repeated interactions on the same topics would help."
                    .to_string(),
            );
        }

        suggestions
    }

    /// Generate improvement suggestions without re-analysing.
    pub fn suggest_improvements(&self) -> Vec<String> {
        let mut out = Vec::new();
        let score = self.metrics.health_score();
        if score < 0.3 {
            out.push(
                "Overall knowledge health is LOW. Priority: ingest more diverse knowledge."
                    .to_string(),
            );
        } else if score < 0.6 {
            out.push(
                "Overall knowledge health is MODERATE. Suggest running consolidation.".to_string(),
            );
        } else {
            out.push("Overall knowledge health is GOOD.".to_string());
        }
        if !self.weak_coverage_topics.is_empty() {
            out.push(format!(
                "Weak topics needing more KUs: {}",
                self.weak_coverage_topics.join(", ")
            ));
        }
        out
    }
}
