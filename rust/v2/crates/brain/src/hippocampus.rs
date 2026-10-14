// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Hippocampus — session memory, episodic memory, consolidation, and style learning.
//!
//! The hippocampus manages the *operational present*: conversation threads,
//! short-term episodic memories, and the learned style vector.  It decides
//! what context to inject into the thalamus for each query and what is
//! ready for consolidation into the long-term graph.

use chrono::{DateTime, Utc};
use df_core::Seed;
use serde::{Deserialize, Serialize};

// ── StyleVector ───────────────────────────────────────────────────────────────

/// Continuously-updated vector describing how the user prefers to communicate.
/// Updated via exponential moving average after each successful exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleVector {
    /// 0.0 = very casual, 1.0 = highly formal
    pub formality: f32,
    /// 0.0 = terse, 1.0 = verbose
    pub verbosity: f32,
    /// 0.0 = lay terms, 1.0 = expert/technical
    pub technicality: f32,
    /// 0.0 = purely analytical, 1.0 = empathetic/emotional
    pub empathy: f32,
    /// Number of exchanges used to compute this vector
    pub sample_count: u64,
}

impl Default for StyleVector {
    fn default() -> Self {
        StyleVector {
            formality: 0.5,    // neutral — neither casual nor formal
            verbosity: 0.5,    // moderate detail
            technicality: 0.3, // slightly accessible
            empathy: 0.0,
            sample_count: 0,
        }
    }
}

impl StyleVector {
    const EMA_ALPHA: f32 = 0.15;

    /// Update the vector from an observed exchange.
    pub fn update(&mut self, formality: f32, verbosity: f32, technicality: f32, empathy: f32) {
        let a = Self::EMA_ALPHA;
        self.formality    = a * formality.clamp(0.0, 1.0)    + (1.0 - a) * self.formality;
        self.verbosity    = a * verbosity.clamp(0.0, 1.0)    + (1.0 - a) * self.verbosity;
        self.technicality = a * technicality.clamp(0.0, 1.0) + (1.0 - a) * self.technicality;
        self.empathy      = a * empathy.clamp(0.0, 1.0)      + (1.0 - a) * self.empathy;
        self.sample_count += 1;
    }
}

// ── ConversationThread ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub query: String,
    pub response: String,
    pub timestamp: DateTime<Utc>,
    /// Seeds of KnowledgeNodes that contributed to this response.
    pub ku_seeds: Vec<Seed>,
    /// Per-KU source attribution: (seed, contribution_pct).
    /// contribution_pct = node_activation_score / total_activation_scores.
    #[serde(default)]
    pub source_attribution: Vec<(Seed, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationThread {
    pub id: String,
    pub turns: Vec<Turn>,
    pub started_at: DateTime<Utc>,
    /// Seeds of the dominant topics in this thread.
    pub topic_seeds: Vec<Seed>,
}

impl ConversationThread {
    pub fn new(id: impl Into<String>) -> Self {
        ConversationThread {
            id: id.into(),
            turns: Vec::new(),
            started_at: Utc::now(),
            topic_seeds: Vec::new(),
        }
    }

    pub fn push_turn(&mut self, query: String, response: String, ku_seeds: Vec<Seed>) {
        self.turns.push(Turn {
            query,
            response,
            timestamp: Utc::now(),
            ku_seeds,
            source_attribution: Vec::new(),
        });
    }

    pub fn push_turn_with_attribution(
        &mut self,
        query: String,
        response: String,
        ku_seeds: Vec<Seed>,
        source_attribution: Vec<(Seed, f32)>,
    ) {
        self.turns.push(Turn {
            query,
            response,
            timestamp: Utc::now(),
            ku_seeds,
            source_attribution,
        });
    }

    pub fn recent_context(&self, n: usize) -> Vec<(&str, &str)> {
        self.turns
            .iter()
            .rev()
            .take(n)
            .rev()
            .map(|t| (t.query.as_str(), t.response.as_str()))
            .collect()
    }
}

// ── EpisodicMemory ────────────────────────────────────────────────────────────

/// A memorable event or exchange — can be consolidated into the graph over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemory {
    pub id: String,
    pub description: String,
    /// Seeds of nodes involved in the episode.
    pub seeds_involved: Vec<Seed>,
    /// -1.0 = very negative, 0.0 = neutral, 1.0 = very positive
    pub emotional_valence: f32,
    pub timestamp: DateTime<Utc>,
    /// True once this episode has been distilled into a long-term KU.
    pub consolidated: bool,
}

impl EpisodicMemory {
    pub fn new(description: impl Into<String>, valence: f32, seeds: Vec<Seed>) -> Self {
        EpisodicMemory {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.into(),
            seeds_involved: seeds,
            emotional_valence: valence.clamp(-1.0, 1.0),
            timestamp: Utc::now(),
            consolidated: false,
        }
    }
}

// ── ConsolidationQueue ────────────────────────────────────────────────────────

/// Buffer of episodes waiting to be distilled into long-term graph nodes.
#[derive(Debug, Default)]
pub struct ConsolidationQueue {
    pub pending: Vec<EpisodicMemory>,
    pub max_size: usize,
}

impl ConsolidationQueue {
    pub fn new(max_size: usize) -> Self {
        ConsolidationQueue {
            pending: Vec::new(),
            max_size,
        }
    }

    pub fn push(&mut self, ep: EpisodicMemory) {
        if self.pending.len() >= self.max_size {
            // Drop oldest when full
            self.pending.remove(0);
        }
        self.pending.push(ep);
    }

    pub fn drain_ready(&mut self) -> Vec<EpisodicMemory> {
        let ready: Vec<EpisodicMemory> = self
            .pending
            .iter()
            .filter(|e| !e.consolidated && e.timestamp < Utc::now())
            .cloned()
            .collect();
        self.pending.retain(|e| e.consolidated);
        ready
    }
}

// ── ConsolidationReport ───────────────────────────────────────────────────────

/// Summary of what happened during a consolidation pass.
#[derive(Debug, Clone)]
pub struct ConsolidationReport {
    /// Number of episodes that were processed.
    pub episodes_processed: usize,
    /// Number of bonds that were strengthened.
    pub bonds_strengthened: usize,
    /// Pairs of node hex strings whose co-occurrence was reinforced.
    pub reinforced_pairs: Vec<(String, String)>,
}

// ── SessionContext ────────────────────────────────────────────────────────────

/// Snapshot of operational context injected into each query pipeline.
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub recent_queries: Vec<String>,
    pub active_topics: Vec<String>,
    pub style: StyleVector,
    pub turn_count: u32,
}

// ── Hippocampus ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Hippocampus {
    pub current_thread: Option<ConversationThread>,
    pub threads: Vec<ConversationThread>,
    pub episodes: Vec<EpisodicMemory>,
    pub style: StyleVector,
    pub consolidation_queue: ConsolidationQueue,
    pub max_context_turns: usize,
}

impl Default for Hippocampus {
    fn default() -> Self {
        Hippocampus::new()
    }
}

impl Hippocampus {
    pub fn new() -> Self {
        Hippocampus {
            current_thread: Some(ConversationThread::new("default")),
            threads: Vec::new(),
            episodes: Vec::new(),
            style: StyleVector::default(),
            consolidation_queue: ConsolidationQueue::new(256),
            max_context_turns: 10,
        }
    }

    pub fn push_turn(&mut self, query: String, response: String, ku_seeds: Vec<Seed>) {
        if let Some(thread) = &mut self.current_thread {
            thread.push_turn(query, response, ku_seeds);
        }
    }

    pub fn push_turn_with_attribution(
        &mut self,
        query: String,
        response: String,
        ku_seeds: Vec<Seed>,
        source_attribution: Vec<(Seed, f32)>,
    ) {
        if let Some(thread) = &mut self.current_thread {
            thread.push_turn_with_attribution(query, response, ku_seeds, source_attribution);
        }
    }

    pub fn context_snapshot(&self) -> SessionContext {
        let thread = self.current_thread.as_ref();
        let recent_queries: Vec<String> = thread
            .map(|t| {
                t.turns
                    .iter()
                    .rev()
                    .take(self.max_context_turns)
                    .map(|turn| turn.query.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let turn_count = thread.map(|t| t.turns.len() as u32).unwrap_or(0);

        SessionContext {
            active_topics: vec![],
            recent_queries,
            style: self.style.clone(),
            turn_count,
        }
    }

    pub fn start_new_thread(&mut self, id: impl Into<String>) {
        if let Some(old) = self.current_thread.take() {
            self.threads.push(old);
        }
        self.current_thread = Some(ConversationThread::new(id));
    }

    pub fn record_episode(&mut self, description: impl Into<String>, valence: f32, seeds: Vec<Seed>) {
        let ep = EpisodicMemory::new(description, valence, seeds.clone());
        self.consolidation_queue.push(ep.clone());
        self.episodes.push(ep);
        // Cap episode history to prevent unbounded memory growth across sessions.
        // Keep the most recent 500 episodes; older ones have already been consolidated.
        if self.episodes.len() > 500 {
            self.episodes.drain(..self.episodes.len() - 500);
        }
    }

    /// How many episodes are pending consolidation.
    pub fn pending_count(&self) -> usize {
        self.consolidation_queue.pending.len()
    }

    /// Run a consolidation pass over all un-consolidated episodes.
    ///
    /// For every pair of nodes that co-occur in ≥2 episodes, the bond weight
    /// between them (if any) is strengthened.  If no direct bond exists the
    /// co-occurrence is still recorded in the report so the caller can create
    /// one.
    ///
    /// The `strengthen_bond` callback receives `(source_hex, target_hex)` and
    /// should increase the weight of the bond between those nodes.
    pub fn consolidate<F>(&mut self, mut strengthen_bond: F) -> ConsolidationReport
    where
        F: FnMut(&str, &str) -> bool,
    {
        let episodes: Vec<&mut EpisodicMemory> = self
            .episodes
            .iter_mut()
            .filter(|e| !e.consolidated)
            .collect();

        if episodes.is_empty() {
            return ConsolidationReport {
                episodes_processed: 0,
                bonds_strengthened: 0,
                reinforced_pairs: vec![],
            };
        }

        // Build co-occurrence counts: (seed_hex_a, seed_hex_b) → count
        // (sorted so the pair is canonical regardless of order)
        let mut co_occur: std::collections::HashMap<(String, String), usize> =
            std::collections::HashMap::new();

        // Snapshot seeds before mutably borrowing episode.consolidated
        let snapshots: Vec<Vec<Seed>> = self
            .episodes
            .iter()
            .filter(|e| !e.consolidated)
            .map(|e| e.seeds_involved.clone())
            .collect();

        for seeds in &snapshots {
            for i in 0..seeds.len() {
                for j in (i + 1)..seeds.len() {
                    let a = seeds[i].to_hex();
                    let b = seeds[j].to_hex();
                    let key = if a < b { (a, b) } else { (b, a) };
                    *co_occur.entry(key).or_insert(0) += 1;
                }
            }
        }

        // Strengthen bonds for pairs with co-occurrence ≥ 2
        let mut bonds_strengthened = 0usize;
        let mut reinforced_pairs = Vec::new();

        for ((hex_a, hex_b), count) in &co_occur {
            if *count >= 2 {
                if strengthen_bond(hex_a, hex_b) {
                    bonds_strengthened += 1;
                }
                reinforced_pairs.push((hex_a.clone(), hex_b.clone()));
            }
        }

        let ep_count = snapshots.len();

        // Mark episodes consolidated
        for ep in self.episodes.iter_mut().filter(|e| !e.consolidated) {
            ep.consolidated = true;
        }
        self.consolidation_queue.pending.iter_mut().for_each(|e| e.consolidated = true);
        self.consolidation_queue.pending.retain(|e| !e.consolidated);

        ConsolidationReport {
            episodes_processed: ep_count,
            bonds_strengthened,
            reinforced_pairs,
        }
    }
}
