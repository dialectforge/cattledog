// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Curiosity & Improvement Engine
//!
//! Gives Lu intrinsic motivation. Instead of passively waiting for queries,
//! the curiosity engine detects knowledge gaps, ranks them by interest,
//! and (when permitted by the autonomy layer) initiates research to fill them.
//!
//! Components:
//! - **Gap Detector**: analyses recall results and activation patterns to find
//!   sparse regions, dead-end traversals, and low-confidence clusters.
//! - **Wonder Queue**: priority queue of curiosity signals ranked by score.
//! - **Interest Profile**: per-domain curiosity weights that evolve over time.
//!
//! All actions respect the DialectForge terminal bond. Gap-filling proposals
//! go through the standard propose/approve flow — curiosity never writes
//! directly to the Right Brain graph.
//!
//! Created by John Dean Martin

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use graph::network::RecallResult;

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("cur_{:08x}", (nanos & 0xFFFFFFFF) as u32)
}

// ━━━ Gap Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What kind of knowledge gap was detected?
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GapType {
    /// Query activated very few nodes — sparse region
    SparseActivation { query: String, activated_count: usize },
    /// Traversal hit dead ends — nodes with no outbound bonds
    DeadEnd { node_hex: String, node_label: String },
    /// Cluster of low-confidence nodes — uncertain knowledge
    LowConfidenceCluster { domain: String, avg_confidence: f32 },
    /// Two domains that should connect but have no cross-bonds
    MissingBridge { domain_a: String, domain_b: String },
    /// A scar exists but no replacement knowledge was ever learned
    UnresolvedScar { scar_reason: String },
    /// Repeated queries in same area with poor results
    RecurringFailure { query_pattern: String, failure_count: u32 },
}

/// A single curiosity signal — something Lu wants to know more about.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CuriositySignal {
    pub id: String,
    pub gap: GapType,
    pub priority_score: f32,
    pub domain: String,
    pub suggested_query: String,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
}

impl PartialEq for CuriositySignal {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}
impl Eq for CuriositySignal {}

impl PartialOrd for CuriositySignal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// BinaryHeap is a max-heap — higher priority_score pops first.
impl Ord for CuriositySignal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority_score
            .partial_cmp(&other.priority_score)
            .unwrap_or(Ordering::Equal)
    }
}

// ━━━ Interest Profile ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Per-domain curiosity weight. Domains Lu is more curious about
/// get higher priority when gaps are detected in that area.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainInterest {
    pub domain: String,
    pub weight: f32,          // 0.0 = no interest, 1.0 = maximum
    pub queries_seen: u32,    // how many queries touched this domain
    pub gaps_found: u32,      // how many gaps detected in this domain
    pub gaps_resolved: u32,   // how many gaps were successfully filled
    pub last_active: DateTime<Utc>,
}

impl DomainInterest {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
            weight: 0.5, // neutral starting curiosity
            queries_seen: 0,
            gaps_found: 0,
            gaps_resolved: 0,
            last_active: Utc::now(),
        }
    }

    /// Curiosity grows when gaps are found and resolved (success breeds interest).
    /// It decays slightly when gaps are found but never resolved (frustration).
    pub fn update_weight(&mut self) {
        let resolve_ratio = if self.gaps_found > 0 {
            self.gaps_resolved as f32 / self.gaps_found as f32
        } else {
            0.5
        };
        // Base: activity level (more queries = more interest)
        let activity = (self.queries_seen as f32 / 100.0).min(0.3);
        // Resolve success: higher ratio = more curiosity
        let success_boost = resolve_ratio * 0.4;
        // Recency: decay if domain hasn't been active
        let age_hours = (Utc::now() - self.last_active).num_hours() as f32;
        let recency = 1.0 / (1.0 + age_hours / 168.0); // half-life ~1 week
        self.weight = (0.1 + activity + success_boost * recency).clamp(0.0, 1.0);
    }
}

// ━━━ Gap Detector ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Thresholds for gap detection.
const SPARSE_THRESHOLD: usize = 3;     // fewer than this = sparse
const LOW_CONFIDENCE_THRESHOLD: f32 = 0.5;
const DEAD_END_MIN_STRENGTH: f32 = 5.0; // only flag strong dead-ends

/// Analyses a RecallResult to detect knowledge gaps.
pub fn detect_gaps(
    result: &RecallResult,
    query: &str,
    domain: &str,
    interests: &HashMap<String, DomainInterest>,
) -> Vec<CuriositySignal> {
    let mut signals = Vec::new();
    let domain_weight = interests
        .get(domain)
        .map(|d| d.weight)
        .unwrap_or(0.5);

    // Gap 1: Sparse activation — query barely lit up the graph
    if result.activated_nodes.len() < SPARSE_THRESHOLD {
        signals.push(CuriositySignal {
            id: short_id(),
            gap: GapType::SparseActivation {
                query: query.to_string(),
                activated_count: result.activated_nodes.len(),
            },
            priority_score: 8.0 * domain_weight, // high priority
            domain: domain.to_string(),
            suggested_query: format!("What is {}?", query),
            created_at: Utc::now(),
            resolved: false,
        });
    }

    // Gap 2: Dead-end nodes — activated nodes with no outgoing traversal
    let traversed_from: std::collections::HashSet<&str> = result
        .traversal_path
        .iter()
        .map(|hop| hop.from.as_str())
        .collect();

    for node in &result.activated_nodes {
        if node.strength >= DEAD_END_MIN_STRENGTH
            && !traversed_from.contains(node.seed_hex.as_str())
        {
            signals.push(CuriositySignal {
                id: short_id(),
                gap: GapType::DeadEnd {
                    node_hex: node.seed_hex.clone(),
                    node_label: node.label.clone(),
                },
                priority_score: 4.0 * domain_weight,
                domain: domain.to_string(),
                suggested_query: format!("What is {}?", query),
                created_at: Utc::now(),
                resolved: false,
            });
        }
    }

    // Gap 3: Low-confidence cluster
    let low_conf_nodes: Vec<&str> = result
        .activated_nodes
        .iter()
        .filter(|n| {
            let conf_val = match n.confidence {
                graph::Confidence::Terminal => 0.95,
                graph::Confidence::Verified => 0.8,
                graph::Confidence::Unverified => 0.4,
                graph::Confidence::Contested => 0.1,
            };
            conf_val < LOW_CONFIDENCE_THRESHOLD
        })
        .map(|n| n.label.as_str())
        .collect();

    if low_conf_nodes.len() >= 2 {
        signals.push(CuriositySignal {
            id: short_id(),
            gap: GapType::LowConfidenceCluster {
                domain: domain.to_string(),
                avg_confidence: LOW_CONFIDENCE_THRESHOLD,
            },
            priority_score: 6.0 * domain_weight,
            domain: domain.to_string(),
            suggested_query: format!(
                "Verify: {}",
                low_conf_nodes.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            ),
            created_at: Utc::now(),
            resolved: false,
        });
    }

    signals
}

// ━━━ Wonder Queue ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The Wonder Queue: a priority queue of things Lu wants to know.
/// Highest-priority gaps surface first.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WonderQueue {
    items: Vec<CuriositySignal>,   // stored as Vec, heapified on demand
    max_size: usize,
    resolved_count: u32,
}

impl WonderQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: Vec::new(),
            max_size,
            resolved_count: 0,
        }
    }

    /// Add new curiosity signals. Deduplicates by suggested_query.
    pub fn push_signals(&mut self, signals: Vec<CuriositySignal>) {
        for signal in signals {
            let already_exists = self.items.iter().any(|s| {
                s.suggested_query == signal.suggested_query && !s.resolved
            });
            if !already_exists {
                self.items.push(signal);
            }
        }
        // Trim to max_size, keeping highest priority
        if self.items.len() > self.max_size {
            self.items.sort_by(|a, b| b.priority_score
                .partial_cmp(&a.priority_score)
                .unwrap_or(Ordering::Equal));
            self.items.truncate(self.max_size);
        }
    }

    /// Pop the highest-priority unresolved signal.
    pub fn pop_next(&mut self) -> Option<CuriositySignal> {
        // Build a heap from unresolved items
        let mut heap: BinaryHeap<CuriositySignal> = self.items
            .iter()
            .filter(|s| !s.resolved)
            .cloned()
            .collect();
        
        if let Some(top) = heap.pop() {
            // Mark it resolved in our storage
            if let Some(item) = self.items.iter_mut().find(|s| s.id == top.id) {
                item.resolved = true;
                self.resolved_count += 1;
            }
            Some(top)
        } else {
            None
        }
    }

    /// Peek at top N unresolved signals without removing them.
    pub fn peek(&self, n: usize) -> Vec<&CuriositySignal> {
        let mut unresolved: Vec<&CuriositySignal> = self.items
            .iter()
            .filter(|s| !s.resolved)
            .collect();
        unresolved.sort_by(|a, b| b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(Ordering::Equal));
        unresolved.into_iter().take(n).collect()
    }

    /// Mark a signal as resolved by ID.
    pub fn resolve(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|s| s.id == id) {
            item.resolved = true;
            self.resolved_count += 1;
            true
        } else {
            false
        }
    }

    /// How many unresolved wonders remain?
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|s| !s.resolved).count()
    }

    /// Total signals ever queued.
    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    /// Cleanup: remove old resolved items to save memory.
    pub fn gc(&mut self) {
        self.items.retain(|s| !s.resolved);
    }
}

// ━━━ Curiosity Engine ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The top-level curiosity engine. Owns the wonder queue and
/// interest profile. Called after every recall to detect gaps
/// and update curiosity state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CuriosityEngine {
    pub wonder_queue: WonderQueue,
    pub interests: HashMap<String, DomainInterest>,
    pub enabled: bool,
    /// When true, recall() will trigger automatic web research to fill
    /// detected gaps. Research runs AFTER the response is returned so
    /// it never blocks the caller. Default: false (opt-in).
    #[serde(default)]
    pub auto_resolve: bool,
    /// How many total gaps have been detected across all time
    pub total_gaps_detected: u64,
    /// How many have been resolved
    pub total_gaps_resolved: u64,
    /// Failure tracker: query patterns that keep failing
    failure_tracker: HashMap<String, u32>,
}

impl CuriosityEngine {
    pub fn new() -> Self {
        Self {
            wonder_queue: WonderQueue::new(200), // max 200 pending wonders
            interests: HashMap::new(),
            enabled: true,
            auto_resolve: false,
            total_gaps_detected: 0,
            total_gaps_resolved: 0,
            failure_tracker: HashMap::new(),
        }
    }

    /// Called after every recall. Analyses the result for gaps and
    /// updates the interest profile for the relevant domain.
    pub fn after_recall(
        &mut self,
        result: &RecallResult,
        query: &str,
        domain: &str,
    ) {
        if !self.enabled {
            return;
        }

        // Update domain interest
        let interest = self.interests
            .entry(domain.to_string())
            .or_insert_with(|| DomainInterest::new(domain));
        interest.queries_seen += 1;
        interest.last_active = Utc::now();

        // Track recurring failures
        if result.activated_nodes.len() < SPARSE_THRESHOLD {
            let key = domain.to_string();
            let count = self.failure_tracker.entry(key.clone()).or_insert(0);
            *count += 1;
        }

        // Detect gaps
        let mut signals = detect_gaps(result, query, domain, &self.interests);

        // Check for recurring failures
        for (pattern, count) in &self.failure_tracker {
            if *count >= 3 {
                signals.push(CuriositySignal {
                    id: short_id(),
                    gap: GapType::RecurringFailure {
                        query_pattern: pattern.clone(),
                        failure_count: *count,
                    },
                    priority_score: 9.0, // recurring failures are highest priority
                    domain: pattern.clone(),
                    suggested_query: format!("Research: {}", pattern),
                    created_at: Utc::now(),
                    resolved: false,
                });
            }
        }

        let gap_count = signals.len() as u64;
        self.total_gaps_detected += gap_count;

        // Update domain gap count
        if let Some(interest) = self.interests.get_mut(domain) {
            interest.gaps_found += gap_count as u32;
            interest.update_weight();
        }

        // Push to wonder queue
        self.wonder_queue.push_signals(signals);
    }

    /// Mark a gap as resolved (e.g., after successful learning fills the gap).
    pub fn resolve_gap(&mut self, signal_id: &str, domain: &str) {
        if self.wonder_queue.resolve(signal_id) {
            self.total_gaps_resolved += 1;
            if let Some(interest) = self.interests.get_mut(domain) {
                interest.gaps_resolved += 1;
                interest.update_weight();
            }
        }
    }

    /// Get the next thing Lu is most curious about.
    pub fn next_wonder(&mut self) -> Option<CuriositySignal> {
        self.wonder_queue.pop_next()
    }

    /// Peek at top N wonders without consuming them.
    pub fn top_wonders(&self, n: usize) -> Vec<&CuriositySignal> {
        self.wonder_queue.peek(n)
    }

    /// Get interest profile for a specific domain.
    pub fn domain_interest(&self, domain: &str) -> Option<&DomainInterest> {
        self.interests.get(domain)
    }

    /// Set curiosity weight for a domain manually.
    pub fn set_interest(&mut self, domain: &str, weight: f32) {
        let interest = self.interests
            .entry(domain.to_string())
            .or_insert_with(|| DomainInterest::new(domain));
        interest.weight = weight.clamp(0.0, 1.0);
    }

    /// Enable/disable the curiosity engine.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Enable/disable automatic wonder resolution via web research.
    pub fn set_auto_resolve(&mut self, enabled: bool) {
        self.auto_resolve = enabled;
    }

    /// Garbage-collect resolved wonders and reset failure tracker.
    pub fn gc(&mut self) {
        self.wonder_queue.gc();
        // Reset failure counts for domains that have been addressed
        self.failure_tracker.retain(|_, count| *count < 3);
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Summary of the curiosity engine state for MCP/terminal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CuriosityStatus {
    pub enabled: bool,
    pub auto_resolve: bool,
    pub pending_wonders: usize,
    pub total_detected: u64,
    pub total_resolved: u64,
    pub top_wonders: Vec<WonderSummary>,
    pub interests: Vec<InterestSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WonderSummary {
    pub id: String,
    pub gap_type: String,
    pub priority: f32,
    pub domain: String,
    pub suggested_query: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterestSummary {
    pub domain: String,
    pub weight: f32,
    pub queries: u32,
    pub gaps_found: u32,
    pub gaps_resolved: u32,
}

impl CuriosityEngine {
    /// Build a status snapshot for display.
    pub fn status(&self) -> CuriosityStatus {
        let top = self.wonder_queue.peek(10);
        let wonders: Vec<WonderSummary> = top.into_iter().map(|s| {
            let gap_type = match &s.gap {
                GapType::SparseActivation { .. } => "sparse",
                GapType::DeadEnd { .. } => "dead_end",
                GapType::LowConfidenceCluster { .. } => "low_confidence",
                GapType::MissingBridge { .. } => "missing_bridge",
                GapType::UnresolvedScar { .. } => "unresolved_scar",
                GapType::RecurringFailure { .. } => "recurring_failure",
            };
            WonderSummary {
                id: s.id.clone(),
                gap_type: gap_type.to_string(),
                priority: s.priority_score,
                domain: s.domain.clone(),
                suggested_query: s.suggested_query.clone(),
            }
        }).collect();

        let mut interests: Vec<InterestSummary> = self.interests.values().map(|d| {
            InterestSummary {
                domain: d.domain.clone(),
                weight: d.weight,
                queries: d.queries_seen,
                gaps_found: d.gaps_found,
                gaps_resolved: d.gaps_resolved,
            }
        }).collect();
        interests.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(Ordering::Equal));

        CuriosityStatus {
            enabled: self.enabled,
            auto_resolve: self.auto_resolve,
            pending_wonders: self.wonder_queue.pending_count(),
            total_detected: self.total_gaps_detected,
            total_resolved: self.total_gaps_resolved,
            top_wonders: wonders,
            interests,
        }
    }
}


// ━━━ Default Impls (for serde(skip)) ━━━━━━━━━━━━━━━━━━━━━━━━

impl Default for WonderQueue {
    fn default() -> Self {
        Self::new(200)
    }
}

impl Default for CuriosityEngine {
    fn default() -> Self {
        Self::new()
    }
}
