// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! brain v2 — the Lu AI brain, redesigned from the ground up.
//!
//! Architecture: ten specialised brain regions orchestrated by a central
//! thalamus message bus.  Each region is a separate module.
//! The `Brain` struct is the top-level entry point.

pub mod amygdala;
pub mod arena;
pub mod brainstem;
pub mod cerebellum;
pub mod code_sandbox;
pub mod curiosity;
pub mod daemon;
pub mod feature_flags;
pub mod frontal;
pub mod hippocampus;
pub mod hypothalamus;
pub mod migration;
pub mod narration;
pub mod occipital;
pub mod parietal;
pub mod proposals;
pub mod rumination;
pub mod self_improvement;
pub mod session_memory;
pub mod temporal;
pub mod thalamus;
pub mod thinking;
pub mod web_research;
pub mod web_safety;
pub mod web_idle;

use df_core::{Seed, TerminalBond};
use encryption::DynamicDictionary;
use graph::bond::BondType;
use graph::knowledge_unit::KnowledgeUnit;
use graph::network::AssociativeNetwork;
use graph::node::Confidence;

// ── ResponseTiming ────────────────────────────────────────────────────────────

/// Breakdown of time spent in each stage of `respond()`.
/// Available after every `respond()` call via `brain.last_timing`.
#[derive(Debug, Clone, Default)]
pub struct ResponseTiming {
    /// Time spent in `keyword_search` (ms).
    pub keyword_search_ms: u64,
    /// Time spent in `spread_activation` (ms).
    pub spread_activation_ms: u64,
    /// Time spent in `narrate` (ms).
    pub narration_ms: u64,
    /// Total wall-clock time from entry to return (ms).
    pub total_ms: u64,
}

impl ResponseTiming {
    fn format(&self) -> String {
        format!(
            "[timing] keyword={}ms  spread={}ms  narrate={}ms  total={}ms",
            self.keyword_search_ms,
            self.spread_activation_ms,
            self.narration_ms,
            self.total_ms,
        )
    }
}

// ── Brain ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Brain {
    // ── Regions ───────────────────────────────────────────────────────────────
    pub frontal:      frontal::FrontalLobe,
    pub temporal:     temporal::TemporalLobe,
    pub parietal:     parietal::ParietalLobe,
    pub occipital:    occipital::OccipitalLobe,
    pub hippocampus:  hippocampus::Hippocampus,
    pub amygdala:     amygdala::Amygdala,
    pub thalamus:     thalamus::Thalamus,
    pub cerebellum:   cerebellum::Cerebellum,
    pub brainstem:    brainstem::Brainstem,
    pub hypothalamus: hypothalamus::Hypothalamus,

    // ── Core systems ──────────────────────────────────────────────────────────
    pub network:    AssociativeNetwork,
    narration:      narration::NarrationEngine,
    /// ReAct-style thinking engine for complex / multi-part queries.
    pub thinking:   thinking::ThinkingEngine,
    /// Curiosity engine — generates wonder items from knowledge gaps.
    pub curiosity:  curiosity::CuriosityEngine,
    /// Idle auto-research controller.
    pub web_idle:   web_idle::IdleResearcher,
    /// Background rumination — discovers latent bonds from episodic co-activation.
    pub rumination: rumination::RuminationEngine,
    terminal_bond:  TerminalBond,
    storage_path:   Option<std::path::PathBuf>,

    // ── Phase 6: performance + diagnostics ───────────────────────────────────
    /// When `true`, timing info is appended to every `respond()` output.
    pub verbose: bool,
    /// Timing breakdown from the most recent `respond()` call.
    pub last_timing: Option<ResponseTiming>,

    // ── Parity gaps ───────────────────────────────────────────────────────────
    /// Proposals queued during think_through() awaiting human approval.
    pub proposals: proposals::PendingProposals,
    /// Session-scoped tasks, beliefs, and learnings.
    pub session: session_memory::SessionState,
    /// Response variant competition arena (off by default).
    pub arena: arena::Arena,
    /// Self-improvement metrics and meta-reflection.
    pub self_improvement: self_improvement::SelfImprovement,

    // ── Phase 7: quality + architecture ──────────────────────────────────────
    /// Runtime feature toggles — checked before each major capability.
    pub flags: feature_flags::FeatureFlags,
    /// Background daemon schedule (caller drives via Brain::tick()).
    pub daemon: daemon::DaemonMode,
    /// Contradictions detected during rumination passes.
    pub contradictions: Vec<rumination::ContradictionRecord>,
    /// Incremented on every respond() call. run_curiosity() is gated to
    /// every 20th call to avoid iterating 1.7M nodes + 1.3M bonds on the
    /// hot path (~400 MB heap churn per question).
    curiosity_call_count: u64,
}

impl Brain {
    // ── Construction ──────────────────────────────────────────────────────────

    pub fn new() -> Self {
        let (terminal_bond, _terminal_seed) = TerminalBond::initiate();
        Brain {
            frontal:      frontal::FrontalLobe::new(),
            temporal:     temporal::TemporalLobe::new(),
            parietal:     parietal::ParietalLobe::default(),
            occipital:    occipital::OccipitalLobe::new(),
            hippocampus:  hippocampus::Hippocampus::new(),
            amygdala:     amygdala::Amygdala::new(),
            thalamus:     thalamus::Thalamus::new(),
            cerebellum:   cerebellum::Cerebellum::new(),
            brainstem:    brainstem::Brainstem::new(),
            hypothalamus: hypothalamus::Hypothalamus::new(),
            network:      AssociativeNetwork::new(),
            narration:    narration::NarrationEngine::new(2048),
            thinking:     thinking::ThinkingEngine::new(),
            curiosity:    curiosity::CuriosityEngine::new(),
            web_idle:     web_idle::IdleResearcher::new(),
            rumination:   rumination::RuminationEngine::default(),
            terminal_bond,
            storage_path: None,
            verbose:      false,
            last_timing:  None,
            proposals:       proposals::PendingProposals::new(64),
            session:         session_memory::SessionState::new(),
            arena:           arena::Arena::new(),
            self_improvement: self_improvement::SelfImprovement::new(),
            flags:           feature_flags::FeatureFlags::new(),
            daemon:          daemon::DaemonMode::new(),
            contradictions:  Vec::new(),
            curiosity_call_count: 0,
        }
    }

    pub fn set_storage(&mut self, path: std::path::PathBuf) {
        self.storage_path = Some(path.clone());
        self.brainstem.storage_path = Some(path);
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    pub fn load(path: &std::path::PathBuf) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;

        // Phase 5+: JSON format.  Fall back to bincode for v1/v2 files (which
        // will fail the FORMAT_VERSION check below and give a clear error).
        let brain_file: brainstem::BrainFile = serde_json::from_slice(&data)
            .or_else(|_| {
                bincode::deserialize::<brainstem::BrainFile>(&data)
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| format!("Failed to deserialise brain file: {}", e))?;

        // Free the raw file buffer immediately — it's ~2 GB and no longer
        // needed now that deserialization is complete.
        drop(data);

        if &brain_file.header.magic != brainstem::MAGIC {
            return Err("Invalid brain file: bad magic bytes (expected LUBR)".to_string());
        }
        if brain_file.header.format_version != brainstem::FORMAT_VERSION {
            return Err(format!(
                "Unsupported format version {} (this binary handles version {})",
                brain_file.header.format_version,
                brainstem::FORMAT_VERSION
            ));
        }

        let mut brain = Brain::new();
        brain.network = brain_file.network;
        brain.storage_path = Some(path.clone());
        brain.brainstem.storage_path = Some(path.clone());

        // ── Restore hippocampus state ─────────────────────────────────────────
        if let Some(snap) = brain_file.hippocampus_snapshot.as_object() {
            if let Some(v) = snap.get("episodes") {
                if let Ok(episodes) = serde_json::from_value(v.clone()) {
                    brain.hippocampus.episodes = episodes;
                }
            }
            if let Some(v) = snap.get("threads") {
                if let Ok(threads) = serde_json::from_value(v.clone()) {
                    brain.hippocampus.threads = threads;
                }
            }
            if let Some(v) = snap.get("style") {
                if let Ok(mut style) = serde_json::from_value::<hippocampus::StyleVector>(v.clone()) {
                    // If the saved style is uninitialized (no samples yet, all zeros),
                    // start from neutral defaults so narration doesn't apply casual framing
                    // before any conversation has shaped the style.
                    if style.sample_count == 0 {
                        style = hippocampus::StyleVector::default();
                    }
                    brain.hippocampus.style = style;
                }
            }
        }

        // ── Restore amygdala scars ────────────────────────────────────────────
        if let Some(snap) = brain_file.amygdala_snapshot.as_object() {
            if let Some(v) = snap.get("scars") {
                if let Ok(scars) = serde_json::from_value(v.clone()) {
                    brain.amygdala.scars = scars;
                }
            }
        }

        // ── Restore autonomy tier ─────────────────────────────────────────────
        if let Some(label) = &brain_file.autonomy_tier_label {
            let tier = match label.as_str() {
                "supervised"    => Some(hypothalamus::AutonomyTier::Supervised),
                "assisted"      => Some(hypothalamus::AutonomyTier::Assisted),
                "collaborative" => Some(hypothalamus::AutonomyTier::Collaborative),
                "autonomous"    => Some(hypothalamus::AutonomyTier::Autonomous),
                _ => None,
            };
            if let Some(t) = tier {
                brain.hypothalamus.set_autonomy_tier(t);
            }
        }

        // ── Restore wonder items ──────────────────────────────────────────────
        for item in brain_file.wonder_items {
            brain.curiosity.wonder_queue.items.push(item);
        }
        brain.curiosity.wonder_queue.total_generated = brain_file.wonder_total_generated;
        brain.curiosity.wonder_queue.total_resolved  = brain_file.wonder_total_resolved;
        brain.curiosity.weak_query_topics            = brain_file.weak_query_topics;

        // ── Restore performance metrics ───────────────────────────────────────
        if let Some(metrics) = brain_file.performance_metrics {
            brain.self_improvement.metrics = metrics;
        }

        // ── Restore self-improvement indexes ─────────────────────────────────
        brain.self_improvement.quality_log = brain_file.quality_log;
        brain.self_improvement.pattern_index = brain_file.pattern_index;
        brain.self_improvement.hot_path_index = brain_file.hot_path_index;
        brain.self_improvement.node_quality_history = brain_file.node_quality_history;

        // ── Phase 3: restore RecallParameters ────────────────────────────────
        if !brain_file.recall_params_snapshot.is_null() {
            if let Ok(params) = serde_json::from_value::<
                self_improvement::RecallParameters
            >(brain_file.recall_params_snapshot) {
                brain.self_improvement.params = params;
            }
        }


        // ── Restore session learnings ─────────────────────────────────────────
        brain.session.learnings = brain_file.session_learnings;

        // ── Restore full session state ────────────────────────────────────────
        // Beliefs: restored with their original TTLs; any already-expired ones
        // will be pruned by the next prune_expired_beliefs() call in respond().
        brain.session.beliefs       = brain_file.session_beliefs;
        brain.session.topic_seeds   = brain_file.session_topic_seeds;
        brain.session.recent_queries = brain_file.session_recent_queries;

        // ── Restore feature flags ─────────────────────────────────────────────
        brain.flags = brain_file.feature_flags;

        // Rebuild inverted keyword index from loaded nodes.
        brain.network.rebuild_keyword_index();

        Ok(brain)
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self
            .storage_path
            .as_ref()
            .ok_or_else(|| "No storage path configured — call set_storage() first".to_string())?;

        let stats = self.network.stats();

        // Serialise hippocampus ephemeral state to JSON
        let hippocampus_snapshot = serde_json::json!({
            "episodes": serde_json::to_value(&self.hippocampus.episodes)
                .unwrap_or(serde_json::Value::Array(vec![])),
            "threads": serde_json::to_value(&self.hippocampus.threads)
                .unwrap_or(serde_json::Value::Array(vec![])),
            "style": serde_json::to_value(&self.hippocampus.style)
                .unwrap_or_default(),
        });

        // Serialise amygdala scars to JSON
        let amygdala_snapshot = serde_json::json!({
            "scars": serde_json::to_value(&self.amygdala.scars)
                .unwrap_or(serde_json::Value::Array(vec![])),
        });

        let header = brainstem::BrainFileHeader {
            magic: *brainstem::MAGIC,
            format_version: brainstem::FORMAT_VERSION,
            schema_version: brainstem::SCHEMA_VERSION,
            created_at: chrono::Utc::now(),
            last_saved: chrono::Utc::now(),
            node_count: stats.total_nodes as u64,
            bond_count: stats.total_bonds as u64,
            checksum: [0u8; 32],
        };

        // Phase 3: serialise RecallParameters for persistence.
        let recall_params_snapshot =
            serde_json::to_value(&self.self_improvement.params).unwrap_or_default();

        // Temporarily move the network out of self to avoid a multi-GB
        // clone.  After serialisation we move it back (see below).
        let network = std::mem::take(&mut self.network);

        let brain_file = brainstem::BrainFile {
            header,
            network,
            hippocampus_snapshot,
            amygdala_snapshot,
            version_history: vec![],
            autonomy_tier_label: Some(
                self.hypothalamus.autonomy_tier.label().to_string()
            ),
            wonder_items: self.curiosity.wonder_queue.items.clone(),
            wonder_total_generated: self.curiosity.wonder_queue.total_generated,
            wonder_total_resolved: self.curiosity.wonder_queue.total_resolved,
            weak_query_topics: self.curiosity.weak_query_topics.clone(),
            performance_metrics: Some(self.self_improvement.metrics.clone()),
            quality_log: self.self_improvement.quality_log.clone(),
            pattern_index: self.self_improvement.pattern_index.clone(),
            hot_path_index: self.self_improvement.hot_path_index.clone(),
            node_quality_history: self.self_improvement.node_quality_history.clone(),
            recall_params_snapshot,
            session_learnings: self.session.learnings.clone(),
            feature_flags: self.flags.clone(),
            session_beliefs: self.session.beliefs.clone(),
            session_topic_seeds: self.session.topic_seeds.clone(),
            session_recent_queries: self.session.recent_queries.clone(),
        };

        // ── Atomic save: write to a temp file, then rename ──────────────
        // This prevents data loss if serialisation is interrupted.
        let tmp_path = path.with_extension("lbr.tmp");

        let write_result = (|| -> Result<(), String> {
            let file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
            let writer = std::io::BufWriter::with_capacity(8 * 1024 * 1024, file);
            serde_json::to_writer(writer, &brain_file).map_err(|e| e.to_string())?;
            Ok(())
        })();

        // Restore the network from brain_file (borrow ended, move is safe).
        let brainstem::BrainFile { network, .. } = brain_file;
        self.network = network;

        // Only replace the real file if the write fully succeeded.
        match write_result {
            Ok(()) => std::fs::rename(&tmp_path, path).map_err(|e| e.to_string()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                Err(format!("Save failed (original file preserved): {}", e))
            }
        }
    }

    // ── Core API ──────────────────────────────────────────────────────────────

    /// Query the brain: classify intent → recall → narrate.
    ///
    /// Complex and MultiPart queries are transparently routed through the
    /// thinking loop (`think_through`) for per-sub-question recall and synthesis.
    ///
    /// Phase 6: timing is recorded in `self.last_timing` after every call.
    /// If `self.verbose` is true, timing info is appended to the response.
    pub fn respond(&mut self, query: &str) -> String {
        let t_total = std::time::Instant::now();
        eprintln!("[respond] START query={:?} nodes={} bonds={}", &query[..query.len().min(80)], self.network.node_count(), self.network.bond_count());

        // ── Session hygiene ───────────────────────────────────────────────────
        self.session.prune_expired_beliefs();
        // Record this query in the rolling window before anything else so that
        // context_snapshot() and recent_queries seeding see the full history.
        self.session.push_query(query);

        let intent  = self.frontal.classify_intent(query);
        let plan    = self.frontal.build_plan(intent);
        eprintln!("[respond] +{}ms after classify_intent/build_plan intent={:?}", t_total.elapsed().as_millis(), std::mem::discriminant(&plan.intent));

        // ── Thinking loop routing ─────────────────────────────────────────────
        if matches!(
            &plan.intent,
            frontal::QueryIntent::Complex { .. } | frontal::QueryIntent::MultiPart { .. }
        ) {
            eprintln!("[respond] +{}ms routing to think_through (Complex/MultiPart)", t_total.elapsed().as_millis());
            let response = self.think_through(query);
            let total = t_total.elapsed().as_millis() as u64;
            self.last_timing = Some(ResponseTiming {
                keyword_search_ms: 0,
                spread_activation_ms: 0,
                narration_ms: 0,
                total_ms: total,
            });
            return if self.verbose {
                format!("{}\n{}", response, self.last_timing.as_ref().unwrap().format())
            } else {
                response
            };
        }

        // Use the topic extracted from the intent for keyword matching.
        let raw_topic = plan.intent.topic();
        let cleaned = raw_topic
            .trim_start_matches("tell me about ")
            .trim_start_matches("explain ")
            .trim_start_matches("describe ")
            .trim_start_matches("a ")
            .trim_start_matches("an ")
            .trim_start_matches("the ")
            .trim_end_matches('?')
            .trim_end_matches('.')
            .trim();
        let cleaned = cleaned
            .trim_end_matches(" works")
            .trim_end_matches(" work");
        let search_key = if cleaned.is_empty() { query.to_string() } else { cleaned.to_string() };

        // ── Fix 1+3: Context-aware query expansion + coreference resolution ────
        // When the current query is vague (short, affirmative opener, or contains
        // unresolved pronoun references like "look into that" / "yes, please"),
        // pull topic keywords from the last 1-2 prior turns and merge them into
        // the search key.  This keeps turns 4-6 on topic after follow-ups.
        let search_key = {
            const CTX_STOP: &[&str] = &[
                "tell", "what", "does", "that", "with", "from", "have", "this",
                "work", "about", "make", "more", "many", "each", "some", "used",
                "when", "where", "both", "either", "while", "after", "before",
                "why", "how", "the", "and", "for", "are", "was", "were", "been",
                "will", "would", "could", "should", "than", "then", "they",
                "their", "there", "please", "look", "into", "think", "know",
                "want", "like", "just", "much", "very", "also", "sure", "yeah",
                "okay", "great", "going", "said", "told", "help", "need",
                "interesting", "curious", "question", "answer", "actually",
            ];
            let q_lower = query.to_lowercase();
            // Count meaningful content words in the derived search key
            let key_content_words = search_key
                .split_whitespace()
                .filter(|w| w.len() > 3 && !CTX_STOP.contains(w))
                .count();
            // Detect affirmative openers ("yes, please…", "sure, go ahead…")
            let is_affirmative_opener = ["yes ", "yes,", "yeah", "sure ", "sure,",
                "ok ", "ok,", "okay", "yep ", "alright"]
                .iter().any(|s| q_lower.starts_with(s));
            // Detect unresolved pronoun references
            let has_unresolved_ref = [
                "look into that", "into that", "about that", "about it",
                "that one", "the thing you", "you mentioned", "you said",
                "you told me", "what about that", "more on that",
                "tell me more", "go deeper", "elaborate on", "expand on",
                "pulling everything", "pulling it all", "everything together",
                "from what you", "from what you've", "most surprising",
                "what you told", "what did you", "the latest research",
                "what does that mean", "what would that mean",
            ].iter().any(|p| q_lower.contains(p));
            // A query is vague if its search key has few content words, OR if it
            // starts with an affirmative opener with few content words, OR if it
            // contains a pronoun/coreference AND the key is still fairly short.
            // Threshold guards prevent over-expansion on rich queries like
            // "You mentioned the brain — can you go deeper?" (content_words ≈ 8+).
            let is_vague = key_content_words <= 4
                || (is_affirmative_opener && key_content_words <= 8)
                || (has_unresolved_ref && key_content_words <= 6);
            if is_vague && self.session.recent_queries.len() >= 2 {
                // Extract content words from the last 2 prior queries
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                let mut prior_kws: Vec<String> = Vec::new();
                for prior_q in self.session.recent_queries.iter().rev().skip(1).take(2) {
                    for w in prior_q.split_whitespace() {
                        let w_clean = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                        if w_clean.len() > 3
                            && !CTX_STOP.contains(&w_clean.as_str())
                            && seen.insert(w_clean.clone())
                        {
                            prior_kws.push(w_clean);
                        }
                    }
                }
                prior_kws.truncate(10);
                if !prior_kws.is_empty() {
                    // When the query is a bare affirmative ("yes, please…") or has
                    // almost no content words, replace the search key entirely with
                    // prior-turn keywords so the vague opener text doesn't pollute
                    // the search (e.g. "emotional response" matching a record label).
                    // For richer vague queries (pronoun refs with some content words),
                    // append prior keywords to carry context forward.
                    let expanded = if is_affirmative_opener || key_content_words <= 2 {
                        prior_kws.join(" ")
                    } else {
                        format!("{} {}", search_key, prior_kws.join(" "))
                    };
                    let sk_dbg: String = search_key.chars().take(40).collect();
                    let ex_dbg: String = expanded.chars().take(80).collect();
                    eprintln!(
                        "[context-expand] vague={} aff={} ref={} key={:?} → {:?}",
                        is_vague, is_affirmative_opener, has_unresolved_ref,
                        sk_dbg, ex_dbg,
                    );
                    expanded
                } else {
                    search_key
                }
            } else {
                search_key
            }
        };

        // ── Session: collect relevant beliefs before recall ────────────────────
        // Collected as owned Strings so the immutable borrow on self.session
        // is released before any mutable operations later in respond().
        let session_belief_stmts: Vec<String> = {
            let beliefs = self.session.relevant_beliefs(&search_key);
            beliefs.into_iter().map(|b| b.statement.clone()).collect()
        };

        // ── Phase 3: snapshot tunable recall parameters ───────────────────────
        let p_top_k  = self.self_improvement.params.top_k;
        let p_depth  = self.self_improvement.params.bond_traversal_depth as usize;

        // ── Phase 6: timed keyword search ─────────────────────────────────────
        eprintln!("[respond] +{}ms starting keyword_search key={:?} top_k={}", t_total.elapsed().as_millis(), &search_key[..search_key.len().min(60)], p_top_k);
        let t_kw = std::time::Instant::now();
        let mut recall = self.network.keyword_search(&search_key, 3, p_top_k);
        if recall.activated_nodes.is_empty() {
            const QUERY_STOP: &[&str] = &[
                "used", "that", "with", "from", "have", "this", "what", "does",
                "work", "about", "tell", "make", "more", "many", "each", "some",
            ];
            let content_words: Vec<String> = search_key.split_whitespace()
                .map(|w| w.trim_end_matches('?').trim_end_matches('.').to_lowercase())
                .filter(|w| w.len() > 3 && !QUERY_STOP.contains(&w.as_str()))
                .collect();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for w in &content_words {
                let partial = self.network.keyword_search(w, 1, 5);
                for node in partial.activated_nodes {
                    let hex = node.seed.to_hex();
                    if seen.insert(hex) {
                        recall.activated_nodes.push(node);
                    }
                }
            }
            // Boost nodes whose raw_text matches ALL content words — these are
            // much more likely to be about the actual multi-word topic.
            if content_words.len() >= 2 {
                for node in recall.activated_nodes.iter_mut() {
                    let raw_lower = node.knowledge_unit.raw_text().to_lowercase();
                    let all_match = content_words.iter().all(|w| raw_lower.contains(w.as_str()));
                    if all_match {
                        node.strength *= 3.0;
                    }
                }
                recall.activated_nodes.sort_by(|a, b| {
                    b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // ── Fix 2: Session memory as recall hint ──────────────────────────────
        // Fold in nodes from the last 2 prior queries at half weight to maintain
        // conversational continuity.  Threshold raised from ≤5 → ≤20 so that
        // vague follow-ups which expanded to a richer search key still receive
        // a session-context boost even when recall is moderate.
        // Left-brain only: nodes are scored, not added to the knowledge graph.
        if recall.activated_nodes.len() <= 20 {
            let prior_queries: Vec<String> = self.session.recent_queries
                .iter()
                .rev()
                // Skip the query we just pushed (the current one)
                .skip(1)
                .filter(|q| !q.is_empty())
                .take(2)
                .cloned()
                .collect();
            if !prior_queries.is_empty() {
                let mut seen: std::collections::HashSet<String> =
                    recall.activated_nodes.iter().map(|n| n.seed.to_hex()).collect();
                for prior_q in prior_queries {
                    let prior_recall = self.network.keyword_search(&prior_q, 2, 5);
                    for mut node in prior_recall.activated_nodes {
                        // Half-weight: prior context is a weaker signal than the current query.
                        node.strength *= 0.5;
                        if seen.insert(node.seed.to_hex()) {
                            recall.activated_nodes.push(node);
                        }
                    }
                }
            }
        }

        // Fix #1: For Comparison queries, also search topic_b separately and merge.
        // Fix #1B: Boost nodes containing BOTH topics so they rank above single-topic hits.
        if let frontal::QueryIntent::Comparison { topic_a, topic_b } = &plan.intent {
            let a_key = topic_a
                .trim_end_matches('?')
                .trim_end_matches('.')
                .trim()
                .to_lowercase();
            let b_key = topic_b
                .trim_end_matches('?')
                .trim_end_matches('.')
                .trim()
                .to_lowercase();
            if !b_key.is_empty() {
                let b_recall = self.network.keyword_search(&b_key, 3, 10);
                let mut seen: std::collections::HashSet<String> =
                    recall.activated_nodes.iter().map(|n| n.seed.to_hex()).collect();
                for node in b_recall.activated_nodes {
                    if seen.insert(node.seed.to_hex()) {
                        recall.activated_nodes.push(node);
                    }
                }
            }
            // Boost nodes containing both terms — these are likely the defining comparison nodes.
            if !a_key.is_empty() && !b_key.is_empty() {
                for node in recall.activated_nodes.iter_mut() {
                    let raw = node.knowledge_unit.raw_text().to_lowercase();
                    if raw.contains(a_key.as_str()) && raw.contains(b_key.as_str()) {
                        node.strength *= 5.0;
                    }
                }
                recall.activated_nodes.sort_by(|a, b| {
                    b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // Fix #2A: For Definition queries, filter to nodes where the topic appears
        // as a whole word in the first 150 characters of the text (primary-subject filter).
        // Whole-word check prevents "syntax" matching "syntax-based" etc.
        //
        // Priority order:
        //   1. KuKind::Definition nodes whose subject matches the topic (exact definitional KUs)
        //   2. Nodes with the topic as a whole word in the first 150 chars
        //   3. Fall back to unfiltered (no narrowing) if neither tier finds anything
        if matches!(plan.intent, frontal::QueryIntent::Definition { .. }) {
            use graph::knowledge_unit::KuKind;
            let topic_lower = search_key.to_lowercase();

            // Nodes whose raw text contains one of these markers are describing what
            // something is NOT — exclude them from definition recall entirely.
            let contrastive_markers = [
                "opposite of", "contrary to", "not to be confused", "contrast with",
                "in contrast to", "as opposed to", "not the same as", "unlike",
                "differs from", "different from",
            ];
            let is_contrastive = |text: &str| -> bool {
                let tl = text.to_lowercase();
                contrastive_markers.iter().any(|m| tl.contains(m))
            };

            // Tier 1: KuKind::Definition with matching subject (never contrastive).
            let by_kind: Vec<_> = recall.activated_nodes.iter()
                .filter(|n| {
                    n.knowledge_unit.kind() == KuKind::Definition
                        && !is_contrastive(n.knowledge_unit.raw_text())
                        && n.knowledge_unit.subject()
                            .map(|s| s.to_lowercase().contains(&topic_lower)
                                  || topic_lower.contains(&s.to_lowercase()))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();

            if !by_kind.is_empty() {
                recall.activated_nodes = by_kind;
            } else {
                // Tier 2: whole-word match in the first 150 chars (never contrastive).
                let is_word_char = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
                let prominent: Vec<_> = recall.activated_nodes.iter()
                    .filter(|n| {
                        if is_contrastive(n.knowledge_unit.raw_text()) { return false; }
                        let text = n.knowledge_unit.raw_text().to_lowercase();
                        let end = {
                            let mut e = text.len().min(150);
                            while !text.is_char_boundary(e) { e -= 1; }
                            e
                        };
                        let lead = &text[..end];
                        let mut found = false;
                        let mut start = 0;
                        while let Some(pos) = lead[start..].find(topic_lower.as_str()) {
                            let abs = start + pos;
                            let before_ok = abs == 0 || !is_word_char(lead[..abs].chars().last().unwrap_or(' '));
                            let after_pos = abs + topic_lower.len();
                            let after_ok  = after_pos >= lead.len() || !is_word_char(lead[after_pos..].chars().next().unwrap_or(' '));
                            if before_ok && after_ok { found = true; break; }
                            start = abs + 1;
                            if start >= lead.len() { break; }
                        }
                        found
                    })
                    .cloned()
                    .collect();
                // Only narrow if we found nodes that are plausibly definitional
                // (i.e. more than one match, or the single match looks definitional).
                // If just one marginal match survived, keep the full unfiltered set.
                if prominent.len() >= 2 {
                    recall.activated_nodes = prominent;
                }
                // If prominent.len() == 1, leave recall unchanged (fall back to full set).
            }

            // Global contrastive guard: for definition queries, strip ALL contrastive
            // nodes from the final recall set regardless of which tier was used.
            // This prevents "opposite of X" Raw/Definition nodes from leaking into
            // compose_paragraph_hints's raw fallback path (which has no contrastive filter).
            recall.activated_nodes.retain(|n| !is_contrastive(n.knowledge_unit.raw_text()));
        }

        let kw_ms = t_kw.elapsed().as_millis() as u64;
        eprintln!("[respond] +{}ms keyword_search done: {}ms, {} nodes recalled", t_total.elapsed().as_millis(), kw_ms, recall.activated_nodes.len());

        // ── Hybrid search: semantic search + RRF merge ───────────────────────
        // Run TF-IDF embedding search in parallel with keyword results, then
        // merge using Reciprocal Rank Fusion.  This finds nodes that share
        // vocabulary meaning with the query even if they don't contain the
        // exact keywords — fixing cases like "bacteria" matching water regs.
        let t_sem = std::time::Instant::now();
        let semantic_recall = self.network.semantic_search(&search_key, p_top_k * 2);
        let sem_ms = t_sem.elapsed().as_millis() as u64;
        eprintln!("[respond] +{}ms semantic_search done: {}ms, {} nodes", t_total.elapsed().as_millis(), sem_ms, semantic_recall.activated_nodes.len());

        if !semantic_recall.activated_nodes.is_empty() {
            // ── Reciprocal Rank Fusion (k=60) ────────────────────────────────
            // RRF score = sum(1 / (k + rank)) across both retrievers.
            // Higher is better.  k=60 is the standard constant.
            const RRF_K: f32 = 60.0;

            let mut rrf_scores: std::collections::HashMap<String, f32> =
                std::collections::HashMap::new();
            let mut node_map: std::collections::HashMap<String, graph::network::ActivatedNode> =
                std::collections::HashMap::new();

            // Score keyword results by their rank position
            for (rank, node) in recall.activated_nodes.iter().enumerate() {
                let hex = node.seed.to_hex();
                *rrf_scores.entry(hex.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f32);
                node_map.entry(hex).or_insert_with(|| node.clone());
            }

            // Score semantic results by their rank position
            for (rank, node) in semantic_recall.activated_nodes.iter().enumerate() {
                let hex = node.seed.to_hex();
                *rrf_scores.entry(hex.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f32);
                node_map.entry(hex).or_insert_with(|| node.clone());
            }

            // Sort by RRF score and rebuild the recall list
            let mut fused: Vec<(String, f32)> = rrf_scores.into_iter().collect();
            fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            fused.truncate(p_top_k);

            recall.activated_nodes = fused
                .into_iter()
                .filter_map(|(hex, rrf_score)| {
                    let mut node = node_map.remove(&hex)?;
                    // Use RRF score as the node strength so the best-fused
                    // results propagate most strongly through spread activation.
                    // Scale up to keep strengths in the same ballpark as before.
                    node.strength = rrf_score * 100.0;
                    Some(node)
                })
                .collect();

            eprintln!("[respond] +{}ms RRF merge done: {} fused nodes", t_total.elapsed().as_millis(), recall.activated_nodes.len());
        }

        // ── Bond filter: intent-based traversal preference ────────────────────
        // Mechanism/Causal queries follow causal chains; Definition queries stay
        // close to hierarchical structure.  All other intents use every bond type
        // (empty filter = untyped, current behaviour).
        let bond_filter: Vec<BondType> = match &plan.intent {
            frontal::QueryIntent::Mechanism { .. } | frontal::QueryIntent::Causal { .. } => {
                vec![BondType::Causes, BondType::Requires, BondType::Sequence]
            }
            frontal::QueryIntent::Definition { .. } => {
                vec![BondType::IsA, BondType::HasPart]
            }
            _ => vec![],
        };

        // ── Phase 6: timed spread activation ──────────────────────────────────
        eprintln!("[respond] +{}ms starting spread_activation depth={} seeds={}", t_total.elapsed().as_millis(), p_depth, recall.activated_nodes.len());
        let t_sa = std::time::Instant::now();
        if !recall.activated_nodes.is_empty() {
            // Give keyword-matched seed nodes a priority boost so they always
            // outrank traversal noise from high-degree hub nodes.
            let seed_bonus: f32 = 10.0;
            for node in recall.activated_nodes.iter_mut() {
                node.strength *= seed_bonus;
            }

            let mut seeds: Vec<(df_core::Seed, f32)> = recall
                .activated_nodes
                .iter()
                .map(|n| (n.seed.clone(), n.strength))
                .collect();

            // ── Session: inject prior-turn topic seeds ────────────────────────
            // Seeds from the previous turn's recall are carried forward so that
            // conversation-local context propagates into the current activation.
            // Injected at 20 % of the current average seed strength so they
            // amplify related nodes without overwhelming the current query's
            // signal.  Left-brain only — no graph writes.
            if !self.session.topic_seeds.is_empty() {
                let current_hexes: std::collections::HashSet<String> =
                    seeds.iter().map(|(s, _)| s.to_hex()).collect();
                let avg_strength = if seeds.is_empty() { 1.0 }
                    else { seeds.iter().map(|(_, s)| *s).sum::<f32>() / seeds.len() as f32 };
                let carry_strength = avg_strength * 0.20;
                for hex in &self.session.topic_seeds {
                    if !current_hexes.contains(hex) {
                        if let Ok(seed) = df_core::Seed::from_hex(hex) {
                            seeds.push((seed, carry_strength));
                        }
                    }
                }
            }

            // Fix #2B: Definition queries always use depth=1 (shallow+precise);
            // all other intents use the tunable depth from RecallParameters.
            let spread_depth: usize = if matches!(plan.intent, frontal::QueryIntent::Definition { .. }) {
                1
            } else {
                p_depth
            };

            // Typed traversal when a bond filter is set; fall back to untyped if
            // it returns fewer than 2 new nodes (the filter may be too narrow for
            // sparse graphs).
            let seed_hexes: std::collections::HashSet<String> =
                seeds.iter().map(|(s, _)| s.to_hex()).collect();
            let deep = if !bond_filter.is_empty() {
                let typed = self.network.typed_recall(&seeds, &bond_filter, spread_depth, p_top_k);
                let new_count = typed.activated_nodes.iter()
                    .filter(|n| !seed_hexes.contains(&n.seed.to_hex()))
                    .count();
                if new_count < 2 {
                    // Typed traversal was too narrow — fall back to untyped.
                    self.network.spread_activation(&seeds, spread_depth, p_top_k)
                } else {
                    typed
                }
            } else {
                self.network.spread_activation(&seeds, spread_depth, p_top_k)
            };

            let mut seen: std::collections::HashSet<String> = seed_hexes;
            for node in deep.activated_nodes {
                if seen.insert(node.seed.to_hex()) {
                    recall.activated_nodes.push(node);
                }
            }

            // Re-sort so keyword matches (boosted) appear before traversal noise.
            recall.activated_nodes.sort_by(|a, b| {
                b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let sa_ms = t_sa.elapsed().as_millis() as u64;
        eprintln!("[respond] +{}ms spread_activation done: {}ms, {} total nodes", t_total.elapsed().as_millis(), sa_ms, recall.activated_nodes.len());

        // ── Amygdala emotional boosts ─────────────────────────────────────────
        self.amygdala.apply_recall_boosts(&mut recall.activated_nodes);

        // ── Session: belief-based node boosting ───────────────────────────────
        // If an active session belief matches the current query topic, give a
        // small strength boost (up to +30 %) to nodes whose raw text contains
        // keywords from that belief.  This keeps conversationally-established
        // context influencing the ranking without promoting anything to the graph
        // (The Wall: session beliefs cannot auto-promote to Right Brain).
        {
            let belief_keywords: Vec<String> = self.session
                .active_beliefs()
                .into_iter()
                .flat_map(|b| {
                    b.statement
                        .split_whitespace()
                        .filter(|w| w.len() >= 4)
                        .map(|w| w.to_lowercase())
                        .collect::<Vec<_>>()
                })
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if !belief_keywords.is_empty() {
                let mut changed = false;
                for node in recall.activated_nodes.iter_mut() {
                    let raw = node.knowledge_unit.raw_text().to_lowercase();
                    let match_count = belief_keywords.iter()
                        .filter(|kw| raw.contains(kw.as_str()))
                        .count();
                    if match_count > 0 {
                        // Cap boost at 1.3× regardless of how many beliefs match.
                        node.strength *= (1.0 + 0.1 * match_count as f32).min(1.3);
                        changed = true;
                    }
                }
                if changed {
                    recall.activated_nodes.sort_by(|a, b| {
                        b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }
        }

        // ── Phase 3: apply per-KU-kind type weights ───────────────────────────
        {
            let weights = &self.self_improvement.params.type_weighting;
            let mut changed = false;
            for node in recall.activated_nodes.iter_mut() {
                let kind_key = format!("{:?}", node.knowledge_unit.kind()).to_lowercase();
                if let Some(&w) = weights.get(&kind_key) {
                    node.strength *= w;
                    changed = true;
                }
            }
            if changed {
                recall.activated_nodes.sort_by(|a, b| {
                    b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // ── Web research fallback ─────────────────────────────────────────────
        // When recall is poor (< 3 nodes) and the query is information-seeking,
        // attempt a quick web lookup so Lu can answer from real sources rather
        // than confidently guessing.  Gated behind flags.web_research and capped
        // at 15 s / 2 pages so it doesn't block the response path for long.
        // Runs BEFORE confidence gating so the gate sees enriched recall.
        let web_research_triggered = self.flags.web_research
            && recall.activated_nodes.len() < 3
            && matches!(
                plan.intent,
                frontal::QueryIntent::Definition { .. }
                    | frontal::QueryIntent::Mechanism { .. }
                    | frontal::QueryIntent::Who { .. }
                    | frontal::QueryIntent::OpenEnded { .. }
            );
        eprintln!("[respond] +{}ms web_research_triggered={} (flags.web_research={}, recall_nodes={})", t_total.elapsed().as_millis(), web_research_triggered, self.flags.web_research, recall.activated_nodes.len());
        if web_research_triggered {
            let t_web = std::time::Instant::now();
            let cfg = web_research::ResearchConfig {
                timeout_secs: 10,
                max_pages: 2,
                max_depth: 1,
                ..web_research::ResearchConfig::default()
            };
            self.web_idle.record_interaction();
            let researcher = web_research::WebResearcher::with_config(cfg);
            let report = researcher.research(&search_key);
            eprintln!("[respond] +{}ms web_research done: {}ms, pages={} facts={}", t_total.elapsed().as_millis(), t_web.elapsed().as_millis(), report.pages_visited, report.facts_extracted);
            for text in report.extracted_texts() {
                self.learn_text(text, graph::node::Confidence::Inferred);
            }
            if report.pages_visited > 0 {
                // Re-run keyword search now that we've ingested fresh knowledge.
                let fresh = self.network.keyword_search(&search_key, 3, p_top_k);
                if fresh.activated_nodes.len() > recall.activated_nodes.len() {
                    recall = fresh;
                }
            }
        }

        // ── Confidence gating — detect poor recall before narrating ──────────
        // Port of the v1 4-signal gating system. If recall is poor, return an
        // honest "I don't know" instead of confidently narrating noise.
        // Evaluated after any web-research enrichment above.
        let knowledge_gap = self.recall_is_poor(query, &search_key, &recall.activated_nodes);

        // ── Phase 6: timed narration ──────────────────────────────────────────
        eprintln!("[respond] +{}ms starting narration knowledge_gap={}", t_total.elapsed().as_millis(), knowledge_gap);
        let t_narr = std::time::Instant::now();
        let response = if knowledge_gap {
            let topic_display = {
                let t = plan.intent.topic();
                if t.is_empty() { query } else { t }
            };
            format!(
                "I don't have strong knowledge about \"{}\". Would you like me to research this?",
                topic_display
            )
        } else {
            // Use search_key (context-expanded) for content-word extraction so that
            // vague follow-ups like "yes, please look into that" still score the
            // recalled nodes against the actual topic (e.g. "music universal culture
            // emotional response") rather than the vague meta-words in the raw query.
            let narrated = self.narration.narrate_with_style(&plan, &recall, &self.hippocampus.style.clone(), &search_key);
            // Safety net: if quality filtering left us with an empty string, fall back to
            // the knowledge-gap message rather than returning blank output.
            if narrated.trim().is_empty() {
                let topic_display = {
                    let t = plan.intent.topic();
                    if t.is_empty() { query } else { t }
                };
                format!(
                    "I don't have strong knowledge about \"{}\". Would you like me to research this?",
                    topic_display
                )
            } else {
                narrated
            }
        };
        let narr_ms = t_narr.elapsed().as_millis() as u64;
        eprintln!("[respond] +{}ms narration done: {}ms", t_total.elapsed().as_millis(), narr_ms);

        // ── Session: log context internally (never shown to user) ─────────────
        // Session beliefs are used upstream for recall boosting. They should NOT
        // appear in the user-facing response — that's internal metadata.
        if !session_belief_stmts.is_empty() {
            let context = session_belief_stmts
                .iter()
                .map(|s| format!("• {}", s))
                .collect::<Vec<_>>()
                .join("\n");
            eprintln!("[session] Active beliefs:\n{}", context);
        }
        // response stays unchanged — no [Session context] prefix.

        let total_ms = t_total.elapsed().as_millis() as u64;

        self.last_timing = Some(ResponseTiming {
            keyword_search_ms: kw_ms,
            spread_activation_ms: sa_ms,
            narration_ms: narr_ms,
            total_ms,
        });

        // ── Session: update topic seeds for the next turn ─────────────────────
        // Store the top-10 recall node hexes so the next turn (or a post-reload
        // first turn) can inject them as weak activation seeds.
        {
            let top_hexes: Vec<String> = recall.activated_nodes
                .iter()
                .take(10)
                .map(|n| n.seed.to_hex())
                .collect();
            self.session.set_topic_seeds(top_hexes);
        }

        let ku_seeds: Vec<Seed> = recall.activated_nodes.iter().map(|n| n.seed.clone()).collect();

        // ── Self-improvement tracking ─────────────────────────────────────────
        self.self_improvement.record_query(
            query,
            &recall.activated_nodes,
            &recall.traversal_path,
        );

        // ── Phase 3: P-controller — nudge RecallParameters every N queries ────
        {
            let metrics_snap = self.self_improvement.metrics.clone();
            let total        = metrics_snap.total_queries;
            self.self_improvement.tuner.maybe_tune(
                &mut self.self_improvement.params,
                &metrics_snap,
                total,
            );
        }

        // ── Curiosity: flag weak queries as recurring failures ─────────────────
        if recall.activated_nodes.len() < 3 {
            self.curiosity.record_weak_query(&search_key);
        }

        // ── Source attribution (flag-guarded) ────────────────────────────────
        let attribution: Vec<(Seed, f32)> = if self.flags.source_attribution {
            let total_activation: f32 = recall.activated_nodes.iter().map(|n| n.strength).sum();
            recall.activated_nodes.iter().map(|n| {
                let pct = if total_activation > 0.0 { n.strength / total_activation } else { 0.0 };
                (n.seed.clone(), pct)
            }).collect()
        } else {
            vec![]
        };

        self.hippocampus.push_turn_with_attribution(
            query.to_string(),
            response.clone(),
            ku_seeds.clone(),
            attribution,
        );

        // ── Episode recording ─────────────────────────────────────────────────
        let word_count = response.split_whitespace().count();
        let valence = if !recall.activated_nodes.is_empty() { 0.5 } else { -0.1 };
        self.hippocampus.record_episode(
            format!("Q: {} → {} words recalled", query, word_count),
            valence,
            ku_seeds,
        );

        // ── StyleVector update (flag-guarded) ─────────────────────────────────
        if self.flags.style_adaptation {
            let formality    = if query.ends_with('?') { 0.4 } else { 0.6 };
            let verbosity    = (word_count as f32 / 80.0).clamp(0.0, 1.0);
            let technicality = if response.contains("algorithm")
                || response.contains("process")
                || response.contains("system")
                || response.contains("function")
            { 0.7 } else { 0.3 };
            self.hippocampus.style.update(formality, verbosity, technicality, 0.1);
        }

        // ── Session: record belief and optional learning ──────────────────────
        {
            let node_count = recall.activated_nodes.len();
            let recall_confidence: f32 = if node_count >= 5 { 0.85 }
                else if node_count >= 3 { 0.6 }
                else { 0.35 };
            self.session.add_belief(
                &format!("Recent topic: {}", search_key),
                recall_confidence,
                600,
            );
            if node_count >= 5 {
                self.session.add_learning(
                    &format!("Good recall on: {}", search_key),
                    None,
                );
            }
        }

        // ── Curiosity: run gap analysis — gated to every 20th call ───────────
        // run_curiosity() allocates notable memory (node_labels + bond_pairs +
        // O(n²) contradiction scan capped at 200 nodes).  Gate at 20 to allow
        // curiosity to fire within a normal conversation window while still
        // amortising the cost across multiple turns.
        self.curiosity_call_count = self.curiosity_call_count.wrapping_add(1);
        eprintln!("[respond] +{}ms curiosity_call_count={} (runs every 20)", t_total.elapsed().as_millis(), self.curiosity_call_count);
        if self.curiosity_call_count % 20 == 0 {
            eprintln!("[respond] +{}ms starting run_curiosity", t_total.elapsed().as_millis());
            self.run_curiosity();
            eprintln!("[respond] +{}ms run_curiosity done", t_total.elapsed().as_millis());
        }

        eprintln!("[respond] +{}ms RETURN kw={}ms spread={}ms narrate={}ms", t_total.elapsed().as_millis(), kw_ms, sa_ms, narr_ms);
        if self.verbose {
            format!("{}\n{}", response, self.last_timing.as_ref().unwrap().format())
        } else {
            response
        }
    }

    /// 4-signal confidence gate — returns `true` when recalled nodes are too
    /// poor a match for `query` / `search_key` to narrate confidently.
    ///
    /// Signals (ported from v1 crates/brain/src/lib.rs):
    ///   1. Keyword coverage  — ≥50 % of query keywords must appear in top-5 nodes
    ///   2. Specificity miss  — named bigram/trigram must not be wholly absent
    ///   3. Source scatter    — top-4 nodes must share at least one meaningful word
    ///   4. Hub contamination — >40 % of top-5 nodes must not be keyword-free
    ///
    /// When the current turn is a continuation of the prior topic, the keyword
    /// coverage threshold is relaxed from 50 % → 20 % and specificity_miss is
    /// suppressed (same relaxation rule as v1).
    fn recall_is_poor(
        &self,
        query: &str,
        search_key: &str,
        nodes: &[graph::network::ActivatedNode],
    ) -> bool {
        // ── Query keyword extraction ──────────────────────────────────────────
        const STOP: &[&str] = &[
            "what", "does", "that", "with", "from", "have", "this",
            "work", "about", "tell", "make", "more", "many", "each",
            "some", "used", "when", "where", "why", "how", "the",
            "and", "for", "are", "was", "were", "been", "being",
            "will", "would", "could", "should", "shall", "than",
            "then", "they", "their", "there",
        ];
        let query_keywords: Vec<String> = search_key
            .to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 3 && !STOP.contains(&w.as_str()))
            .collect();

        // No keywords → nothing to gate against.
        if query_keywords.is_empty() {
            return false;
        }
        // Empty recall is always a gap.
        if nodes.is_empty() {
            return true;
        }

        // ── Node-count proxy for NarrationConfidence ─────────────────────────
        // ≥5 nodes ≈ Strong; <3 nodes ≈ Weak/Empty; ≥15 nodes ≈ Very Strong.
        let node_count = nodes.len();
        let is_strong        = node_count >= 5;
        let is_very_strong   = node_count >= 15;  // large graph activation — trust it
        let is_weak_or_empty = node_count < 3;

        // ── Signal 1: Keyword coverage ────────────────────────────────────────
        // Also applies stem/prefix matching for long keywords (>= 7 chars):
        // a node word is accepted if the keyword starts with that node word.
        // E.g. "neural" in a node matches query keyword "neurologically".
        let keyword_coverage: f32 = {
            let matched = query_keywords.iter()
                .filter(|kw| {
                    nodes.iter().take(5).any(|n| {
                        let label_lower = n.label.to_lowercase();
                        let text_lower  = n.knowledge_unit.raw_text().to_lowercase();
                        if label_lower.contains(kw.as_str()) || text_lower.contains(kw.as_str()) {
                            return true;
                        }
                        // Stem match for long keywords: accept if any 4+ char word in
                        // the node text/label is a prefix of the keyword.
                        if kw.len() >= 7 {
                            let check = |haystack: &str| {
                                haystack
                                    .split(|c: char| !c.is_alphanumeric())
                                    .any(|nw| nw.len() >= 4 && kw.starts_with(nw))
                            };
                            return check(&label_lower) || check(&text_lower);
                        }
                        false
                    })
                })
                .count();
            matched as f32 / query_keywords.len() as f32
        };

        // ── Conversation continuity relaxation ────────────────────────────────
        // If the prior turn's recalled seeds overlap current top-4 nodes we are
        // most likely continuing the same topic — relax the keyword threshold.
        let in_continuous_convo: bool = self
            .hippocampus
            .current_thread
            .as_ref()
            .and_then(|t| t.turns.last())
            .map(|last| {
                let prior_hexes: std::collections::HashSet<String> =
                    last.ku_seeds.iter().map(|s| s.to_hex()).collect();
                nodes.iter().take(4).any(|n| prior_hexes.contains(&n.seed.to_hex()))
            })
            .unwrap_or(false);

        // Scale keyword-coverage threshold with query length: longer queries
        // naturally spread keywords across more nodes, so we require less coverage.
        // In a continuous conversation trust the topic-chain even more.
        let effective_kw_threshold = if in_continuous_convo {
            0.15_f32
        } else {
            match query_keywords.len() {
                0..=3 => 0.50_f32,
                4     => 0.35_f32,
                5..=6 => 0.25_f32,
                _     => 0.20_f32,   // 7+ keywords — very lenient
            }
        };

        // ── Signal 2: Specificity miss ────────────────────────────────────────
        // Build bigrams (tokens ≥5 chars) and trigrams (tokens ≥4 chars) from
        // the raw query, then check whether the named concept is wholly absent
        // from the top-5 nodes.
        let query_tokens: Vec<String> = query
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_string())
            .collect();

        let mut specific_phrases: Vec<String> = Vec::new();
        if query_tokens.len() >= 2 {
            for w in query_tokens.windows(2) {
                if w[0].len() >= 5 && w[1].len() >= 5 {
                    specific_phrases.push(format!("{} {}", w[0], w[1]));
                }
            }
        }
        if query_tokens.len() >= 3 {
            for w in query_tokens.windows(3) {
                if w[0].len() >= 4 && w[1].len() >= 4 && w[2].len() >= 4 {
                    specific_phrases.push(format!("{} {} {}", w[0], w[1], w[2]));
                }
            }
        }

        let effective_specificity_miss =
            !in_continuous_convo
            && !is_very_strong       // 15+ nodes is strong evidence — skip phrase check
            && !specific_phrases.is_empty()
            && {
                // Part A: no phrase appears in any top-5 node at all.
                let all_phrases_absent = !specific_phrases.iter().any(|phrase| {
                    nodes.iter().take(5).any(|n| {
                        n.label.to_lowercase().contains(phrase.as_str())
                            || n.knowledge_unit.raw_text().to_lowercase().contains(phrase.as_str())
                    })
                });
                // Part B: at least one constituent token is absent from ALL top-5.
                // Guards against common sub-words inflating apparent coverage.
                all_phrases_absent && specific_phrases.iter().any(|phrase| {
                    phrase.split_whitespace().any(|token| {
                        nodes.iter().take(5).all(|n| {
                            !n.label.to_lowercase().contains(token)
                                && !n.knowledge_unit.raw_text().to_lowercase().contains(token)
                        })
                    })
                })
            };

        // ── Signal 3: Source scatter ──────────────────────────────────────────
        // If no pair among the top-4 nodes shares a meaningful word (>4 chars)
        // the activation was scattered via hub traversal — likely noise.
        let top_wordsets: Vec<std::collections::HashSet<String>> = nodes
            .iter()
            .take(4)
            .map(|n| {
                n.label
                    .to_lowercase()
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                    .filter(|w| w.len() > 4)
                    .collect::<std::collections::HashSet<String>>()
            })
            .collect();
        let source_scatter: bool = top_wordsets.len() >= 3 && {
            let any_pair_shares = top_wordsets.iter().enumerate().any(|(i, ws_i)| {
                top_wordsets.iter().enumerate().any(|(j, ws_j)| {
                    j > i && ws_i.intersection(ws_j).count() >= 1
                })
            });
            !any_pair_shares
        };

        // ── Signal 4: Hub contamination ───────────────────────────────────────
        // If >40 % of the top-5 recalled nodes contain none of the query
        // keywords they were likely surfaced via hub traversal, not direct recall.
        let check_n = node_count.min(5).max(1);
        let hub_contaminated_n = nodes
            .iter()
            .take(check_n)
            .filter(|n| {
                !query_keywords.iter().any(|kw| {
                    n.label.to_lowercase().contains(kw.as_str())
                        || n.knowledge_unit.raw_text().to_lowercase().contains(kw.as_str())
                })
            })
            .count();
        // Hub contamination only signals a problem when keyword coverage is also
        // poor (≤0.25). If the top-5 nodes DO contain the query keywords, a high
        // contamination ratio just means hub traversal picked up related material —
        // that's fine.
        let hub_contamination =
            check_n >= 2
            && hub_contaminated_n as f32 / check_n as f32 > 0.40
            && keyword_coverage <= 0.25;

        // ── Final gate ────────────────────────────────────────────────────────
        // A very large activation (≥15 nodes) is strong evidence the graph found
        // something relevant — never reject it based on keyword heuristics alone.
        if is_very_strong {
            return false;
        }

        (keyword_coverage < effective_kw_threshold && is_weak_or_empty)
            || effective_specificity_miss
            || (source_scatter && !is_strong)
            || (hub_contamination && !is_strong)
    }

    /// Run a hippocampus consolidation pass.
    pub fn consolidate(&mut self) -> hippocampus::ConsolidationReport {
        let bond_map: std::collections::HashMap<(String, String), Seed> = self
            .network
            .all_bonds()
            .into_iter()
            .map(|b| {
                let a = b.source_seed.to_hex();
                let c = b.target_seed.to_hex();
                let key = if a <= c { (a, c) } else { (c, a) };
                (key, b.seed.clone())
            })
            .collect();

        let mut pairs_to_strengthen: Vec<Seed> = Vec::new();
        let report = self.hippocampus.consolidate(|hex_a, hex_b| {
            let key = if hex_a <= hex_b {
                (hex_a.to_string(), hex_b.to_string())
            } else {
                (hex_b.to_string(), hex_a.to_string())
            };
            if let Some(bs) = bond_map.get(&key) {
                pairs_to_strengthen.push(bs.clone());
                true
            } else {
                false
            }
        });

        for bond_seed in &pairs_to_strengthen {
            if let Some(bond) = self.network.get_bond_mut(bond_seed) {
                bond.strengthen(0.1);
            }
        }

        // Track consolidation effectiveness.
        self.self_improvement.record_consolidation(report.bonds_strengthened);

        // After consolidation, run a curiosity pass to surface knowledge gaps.
        self.run_curiosity();

        report
    }

    /// Run the curiosity engine: analyse episodes and graph state to generate
    /// wonder items.  Returns the number of new questions generated.
    pub fn run_curiosity(&mut self) -> usize {
        let episode_descs: Vec<String> = self
            .hippocampus
            .episodes
            .iter()
            .rev()
            .take(50)
            .map(|e| e.description.clone())
            .collect();

        // Use iterators with take() to avoid collecting all 1.7M nodes / 1.4M bonds
        // into intermediate Vecs. Caps reduced from 2000 to 500 — sufficient for
        // gap analysis without the O(n²) strategies causing issues.
        let node_labels: Vec<(String, String)> = self
            .network
            .iter_active_nodes()
            .take(500)
            .map(|n| {
                (
                    n.knowledge_unit.raw_text().chars().take(120).collect(),
                    format!("{:?}", n.knowledge_unit.kind()).to_lowercase(),
                )
            })
            .collect();

        let bond_pairs: Vec<(String, String)> = self
            .network
            .iter_bonds()
            .take(500)
            .filter_map(|b| {
                let src = self.network.get_node(&b.source_seed)?;
                let tgt = self.network.get_node(&b.target_seed)?;
                Some((
                    src.knowledge_unit.raw_text().chars().take(60).collect(),
                    tgt.knowledge_unit.raw_text().chars().take(60).collect(),
                ))
            })
            .collect();

        let scar_labels: Vec<String> = self
            .amygdala
            .scars
            .iter()
            .map(|s| s.label.clone())
            .collect();

        let weak_topics = self.curiosity.weak_query_topics.clone();

        self.curiosity.analyze_extended(
            &episode_descs,
            &node_labels,
            &bond_pairs,
            &scar_labels,
            &weak_topics,
        )
    }

    /// Run a background rumination pass: discover latent bonds between nodes that
    /// co-appear in episodic memories.
    ///
    /// Phase 7: also runs contradiction detection, KU promotion, and bond decay
    /// (each guarded by the relevant feature flag).
    ///
    /// Returns an empty `RuminationReport` immediately when called before the
    /// configured cooldown (default 30 s) has elapsed since the last pass.
    pub fn ruminate(&mut self) -> rumination::RuminationReport {
        if !self.flags.rumination {
            return rumination::RuminationReport::default();
        }
        let episodes: Vec<hippocampus::EpisodicMemory> =
            self.hippocampus.episodes.clone();
        let enable_contradiction = self.flags.contradiction_detection;
        let report = self.rumination.step(
            &mut self.network,
            &episodes,
            &mut self.temporal,
            enable_contradiction,
        );
        // Store any newly detected contradictions in the Brain.
        self.contradictions.extend(report.contradictions.iter().cloned());
        report
    }

    /// Build a `DynamicDictionary` populated with terms from the knowledge graph.
    pub fn build_dictionary(&self) -> DynamicDictionary {
        let dict_seed = self.terminal_bond.ai_seed().derive(b"dynamic_dict");
        let mut dict = DynamicDictionary::new(dict_seed);

        const STOP: &[&str] = &[
            "the", "a", "an", "is", "are", "was", "were", "be", "been",
            "and", "or", "but", "in", "on", "at", "to", "for", "of",
            "it", "its", "by", "from", "with", "that", "this", "which",
            "as", "not", "also", "can", "may", "will", "has", "have",
            "had", "use", "used", "uses", "both", "more", "than",
        ];

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for node in self.network.active_nodes() {
            let text = node.knowledge_unit.raw_text().to_lowercase();
            for word in text.split(|c: char| !c.is_alphanumeric()) {
                let w = word.trim();
                if w.len() >= 4 && !STOP.contains(&w) && seen.insert(w.to_string()) {
                    dict.register_str(w);
                }
            }
        }
        dict
    }

    /// Teach a typed KnowledgeUnit directly into the graph.
    pub fn learn(&mut self, ku: KnowledgeUnit, confidence: Confidence) -> Seed {
        let node = graph::node::KnowledgeNode::new(ku, confidence);
        self.network.add_node(node)
    }

    /// Teach an image directly from a file path. No proposal/approval required.
    ///
    /// The occipital lobe encodes the image into a 384-dim embedding. A
    /// `KnowledgeUnit::Image` node is created with that embedding stored in
    /// `semantic_embedding`. The parietal lobe caches the embedding and
    /// creates `Depicts` bonds to any nearby text nodes (similarity ≥ 0.3,
    /// up to 3 bonds) so cross-modal recall can reach this image.
    pub fn learn_image(&mut self, path: &str, caption: Option<&str>) -> Result<Seed, String> {
        let encoding = self.occipital.encode_image(path)?;

        let ocr_text = run_ocr(path);

        let mut text_parts = vec![format!("Image: {}", path)];
        if let Some(c) = caption {
            text_parts.push(format!("Caption: {}", c));
        }
        if let Some(ref t) = ocr_text {
            text_parts.push(format!("OCR: {}", t));
        }
        let ku = KnowledgeUnit::Raw {
            text: text_parts.join("\n"),
            source: None,
        };

        let mut node = graph::node::KnowledgeNode::new(ku, Confidence::Terminal);
        node.semantic_embedding = Some(encoding.embedding.clone());

        let seed = self.network.add_node(node);

        // ── Cross-modal bonding via parietal ──────────────────────────────────
        // Store the embedding in the parietal cache under this seed, then find
        // nearby text nodes and bond them with Depicts so spreading activation
        // can traverse from text queries to this image node and vice-versa.
        self.parietal.embedding_cache.insert(seed.to_hex(), encoding.embedding.clone());

        let neighbors = self.parietal.nearest_neighbors(&encoding.embedding, 5);
        for (neighbor_hex, sim) in neighbors {
            if sim < 0.3 { continue; }
            if let Ok(neighbor_seed) = df_core::Seed::from_hex(&neighbor_hex) {
                let bond = graph::bond::TypedBond::new(
                    seed.clone(),
                    neighbor_seed,
                    BondType::Depicts,
                    sim,
                );
                self.network.add_bond(bond);
            }
        }

        Ok(seed)
    }

    /// Teach raw text — the temporal lobe extracts KUs automatically.
    pub fn learn_text(&mut self, text: &str, confidence: Confidence) -> Vec<Seed> {
        let ctx = temporal::ExtractionContext::new(text);
        let result = self.temporal.extract(&ctx);

        let subjects: Vec<Option<String>> = result
            .units
            .iter()
            .map(|ku| ku.subject().map(normalize_subject))
            .collect();

        let seeds: Vec<Seed> = result
            .units
            .into_iter()
            .map(|ku| {
                let node = graph::node::KnowledgeNode::new(ku, confidence);
                self.network.add_node(node)
            })
            .collect();

        // ── Adjacency SEQUENCE bonds ──────────────────────────────────────────
        for pair in seeds.windows(2) {
            let bond = graph::bond::TypedBond::new(
                pair[0].clone(),
                pair[1].clone(),
                BondType::Sequence,
                1.0,
            );
            self.network.add_bond(bond);
        }

        // ── Shared-subject Context bonds ──────────────────────────────────────
        for i in 0..seeds.len() {
            for j in (i + 1)..seeds.len() {
                if let (Some(ref si), Some(ref sj)) = (&subjects[i], &subjects[j]) {
                    if subjects_overlap(si, sj) {
                        let bond = graph::bond::TypedBond::new(
                            seeds[i].clone(),
                            seeds[j].clone(),
                            BondType::Context,
                            0.8,
                        );
                        self.network.add_bond(bond);
                    }
                }
            }
        }

        seeds
    }

    /// Create a typed bond between two nodes.
    pub fn associate(
        &mut self,
        source: &Seed,
        target: &Seed,
        bond_type: BondType,
        weight: f32,
    ) -> Result<Seed, String> {
        let bond = graph::bond::TypedBond::new(source.clone(), target.clone(), bond_type, weight);
        Ok(self.network.add_bond(bond))
    }

    /// Recall nodes relevant to a query, optionally filtered by bond type.
    pub fn recall(
        &mut self,
        query: &str,
        depth: usize,
        top_k: usize,
    ) -> graph::network::RecallResult {
        self.network.keyword_search(query, depth, top_k)
    }

    /// Revoke a node — severs paths and creates an amygdala scar.
/// CattleDog: implant a semantic (embedding-similarity) scar over an action description.
    /// Wires the dormant ScarV2 fingerprint path. NOTE: the only working text embedder is
    /// embedding::StubEngine (lexical/bag-of-words FNV projection), so similarity here is
    /// vocabulary-overlap, not true semantic paraphrase. Documented limitation.
    pub fn semantic_scar_action(&mut self, text: &str, reason: &str) {
        let emb = embedding::semantic_embed(text);
        self.amygdala.record_scar(df_core::Seed::generate(), text, reason, Some(emb));
    }

    /// CattleDog: check candidate text against all semantic scars. Wires amygdala::check_proposal
    /// (the dormant ScarV2::check_similarity trigger) into a live, callable path. Returns the
    /// triggered scar descriptions (label + reason + similarity).
    pub fn semantic_scar_check(&mut self, text: &str, threshold: Option<f32>) -> Vec<String> {
        if let Some(t) = threshold { self.amygdala.scar_similarity_threshold = t; }
        let emb = embedding::semantic_embed(text);
        self.amygdala.check_proposal(&emb)
    }
    pub fn revoke(&mut self, seed: &Seed, reason: &str) {
        if let Some(node) = self.network.get_node_mut(seed) {
            let label = node.label();
            let embedding = node.semantic_embedding.clone();
            df_core::SeedGated::revoke(node, reason);
            self.amygdala.record_scar(seed.clone(), label, reason, embedding);
        }
    }

    // ── Thinking loop ─────────────────────────────────────────────────────────

    /// Run the full multi-agent thinking pipeline for a complex query.
    ///
    /// Phase 7: when `flags.multi_agent_thinking` is enabled, the pipeline is:
    ///   1. Decompose → sub-questions
    ///   2. RecallAgent per sub-Q  (tight depth-2, top-5)
    ///   3. SynthesisAgent          (unified answer)
    ///   4. CriticAgent             (quality score + issues)
    ///   5. If score < 0.5: retry with CriticAgent suggestions
    ///   6. Return final answer
    ///
    /// Falls back to the original single-pass synthesis when the flag is off.
    pub fn think_through(&mut self, query: &str) -> String {
        if self.flags.multi_agent_thinking {
            self.think_through_multi_agent(query)
        } else {
            self.think_through_simple(query)
        }
    }

    /// Multi-agent thinking pipeline (System 2 reasoning).
    fn think_through_multi_agent(&mut self, query: &str) -> String {
        let sub_questions = self.thinking.decompose(query);
        let max = self.thinking.max_steps.min(sub_questions.len());

        // ── RecallAgent phase ─────────────────────────────────────────────────
        let recall_results = self.run_recall_agents(&sub_questions[..max]);

        // ── SynthesisAgent phase ──────────────────────────────────────────────
        let synthesized =
            thinking::SynthesisAgent::synthesize(query, &recall_results, &self.thinking);

        // ── CriticAgent phase ─────────────────────────────────────────────────
        let critique = thinking::CriticAgent::critique(query, &synthesized, &recall_results);

        let final_answer = if critique.score < 0.5 && !critique.suggestions.is_empty() {
            // ── Retry with suggestions ────────────────────────────────────────
            let retry_query = format!(
                "{} (also address: {})",
                query,
                critique.suggestions.join("; ")
            );
            let retry_sub_qs = self.thinking.decompose(&retry_query);
            let retry_max    = self.thinking.max_steps.min(retry_sub_qs.len());
            let retry_results = self.run_recall_agents(&retry_sub_qs[..retry_max]);
            thinking::SynthesisAgent::synthesize(query, &retry_results, &self.thinking)
        } else {
            synthesized
        };

        self.record_thinking_episode(query, sub_questions.len(), &final_answer);
        final_answer
    }

    /// Original single-pass synthesis (System 1 reasoning / fallback).
    fn think_through_simple(&mut self, query: &str) -> String {
        let sub_questions = self.thinking.decompose(query);
        let max = self.thinking.max_steps.min(sub_questions.len());
        let mut observations: Vec<(String, String)> = Vec::new();

        for sub_q in &sub_questions[..max] {
            let obs = self.observe_for_sub_q(sub_q, 2, 15, 10);
            observations.push((sub_q.clone(), obs));
        }

        let final_answer = self.thinking.synthesize(query, &observations);
        self.record_thinking_episode(query, sub_questions.len(), &final_answer);
        final_answer
    }

    /// Run RecallAgent for each sub-question; returns one result per sub-Q.
    fn run_recall_agents(&mut self, sub_questions: &[String]) -> Vec<thinking::RecallAgentResult> {
        sub_questions
            .iter()
            .map(|sub_q| {
                let (obs, top_nodes) = self.observe_with_nodes(
                    sub_q,
                    thinking::RecallAgent::DEPTH,
                    thinking::RecallAgent::TOP_K,
                    thinking::RecallAgent::TOP_K,
                );
                thinking::RecallAgent::run(sub_q, obs, top_nodes)
            })
            .collect()
    }

    /// Retrieve and narrate for a sub-question; returns (narration, top_nodes).
    fn observe_with_nodes(
        &mut self,
        sub_q: &str,
        depth: usize,
        top_k_kw: usize,
        top_k_sa: usize,
    ) -> (String, Vec<(String, f32)>) {
        let mut sub_recall = self.network.keyword_search(sub_q, depth, top_k_kw);

        if !sub_recall.activated_nodes.is_empty() {
            let seeds: Vec<(Seed, f32)> = sub_recall
                .activated_nodes
                .iter()
                .map(|n| (n.seed.clone(), n.strength))
                .collect();
            let deep = self.network.spread_activation(&seeds, depth, top_k_sa);
            let mut seen: std::collections::HashSet<String> = sub_recall
                .activated_nodes
                .iter()
                .map(|n| n.seed.to_hex())
                .collect();
            for node in deep.activated_nodes {
                if seen.insert(node.seed.to_hex()) {
                    sub_recall.activated_nodes.push(node);
                }
            }
        }

        self.amygdala.apply_recall_boosts(&mut sub_recall.activated_nodes);

        let top_nodes: Vec<(String, f32)> = sub_recall
            .activated_nodes
            .iter()
            .take(top_k_kw)
            .map(|n| (n.label.clone(), n.strength))
            .collect();

        let sub_intent = self.frontal.classify_intent(sub_q);
        let sub_plan   = self.frontal.build_plan(sub_intent);
        let obs        = self.narration.narrate(&sub_plan, &sub_recall, sub_q);

        (obs, top_nodes)
    }

    /// Retrieve and narrate for a sub-question; returns narration string only.
    fn observe_for_sub_q(
        &mut self,
        sub_q: &str,
        depth: usize,
        top_k_kw: usize,
        top_k_sa: usize,
    ) -> String {
        self.observe_with_nodes(sub_q, depth, top_k_kw, top_k_sa).0
    }

    /// Record the thinking episode and update style vector.
    fn record_thinking_episode(&mut self, query: &str, sub_q_count: usize, answer: &str) {
        let word_count = answer.split_whitespace().count();
        self.hippocampus.record_episode(
            format!("Q: {} → thinking chain ({} sub-questions)", query, sub_q_count),
            0.5,
            vec![],
        );
        let verbosity = (word_count as f32 / 80.0).clamp(0.0, 1.0);
        self.hippocampus.style.update(0.4, verbosity, 0.5, 0.1);
    }

    // ── Daemon ────────────────────────────────────────────────────────────────

    /// Enable and start the background daemon.
    ///
    /// Equivalent to setting `flags.daemon_mode = true` and calling
    /// `daemon.start()`.  The caller must drive the daemon by calling
    /// `tick()` in their event loop.
    pub fn start_daemon(&mut self) {
        self.flags.daemon_mode = true;
        self.daemon.start();
    }

    /// Drive the background daemon.  Should be called from the host
    /// application's event loop (terminal REPL, Tauri, MCP server, etc.).
    ///
    /// A no-op when `flags.daemon_mode` is false or `daemon.running` is false.
    ///
    /// Returns a short summary string of actions taken (empty if none).
    pub fn tick(&mut self) -> String {
        if !self.flags.daemon_mode {
            return String::new();
        }

        let actions = self.daemon.tick();
        if actions.is_empty() {
            return String::new();
        }

        let mut log = Vec::new();

        for action in actions {
            match action {
                daemon::DaemonAction::Ruminate => {
                    let report = self.ruminate();
                    if report.bonds_discovered > 0 || report.bonds_strengthened > 0 {
                        log.push(format!(
                            "[daemon] ruminate: +{} bonds, strengthened {}",
                            report.bonds_discovered, report.bonds_strengthened
                        ));
                    }
                    if !report.contradictions.is_empty() {
                        log.push(format!(
                            "[daemon] {} contradiction(s) detected",
                            report.contradictions.len()
                        ));
                    }
                }
                daemon::DaemonAction::IdleResearch => {
                    if let Some(query) = self.web_idle_check() {
                        log.push(format!("[daemon] idle research: {:?}", query));
                    }
                }
                daemon::DaemonAction::Consolidate => {
                    let report = self.consolidate();
                    log.push(format!(
                        "[daemon] consolidate: {} bonds strengthened",
                        report.bonds_strengthened
                    ));
                }
                daemon::DaemonAction::SelfReflect => {
                    let suggestions = self.self_improvement.suggest_improvements();
                    let score       = self.self_improvement.metrics.health_score();
                    log.push(format!(
                        "[daemon] reflect: score={:.2}, {} suggestion(s)",
                        score, suggestions.len()
                    ));
                }
                daemon::DaemonAction::HealthReport => {
                    let h = self.health();
                    log.push(format!(
                        "[daemon] health: nodes={} bonds={} scars={}",
                        h.node_count, h.bond_count, h.scar_count
                    ));
                }
            }
        }

        log.join("\n")
    }

    // ── V1 migration ──────────────────────────────────────────────────────────

    /// Migrate a v1 brain into this v2 brain.
    ///
    /// `v1_path` may be:
    ///   - A directory containing `brain_state.json`
    ///   - A direct path to a v1 JSON brain file
    ///
    /// Returns `Err` if the file cannot be read or parsed; returns `Ok` with
    /// stats even if individual nodes/bonds are skipped.
    pub fn migrate(&mut self, v1_path: &std::path::Path) -> Result<migration::MigrationStats, String> {
        // Resolve the JSON file path.
        let json_path = if v1_path.is_dir() {
            let candidate = v1_path.join("brain_state.json");
            if candidate.exists() {
                candidate
            } else {
                return Err(format!(
                    "No brain_state.json found in '{}'",
                    v1_path.display()
                ));
            }
        } else {
            v1_path.to_path_buf()
        };

        let json_str = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("Failed to read '{}': {}", json_path.display(), e))?;

        let json: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse v1 JSON: {}", e))?;

        self.migrate_from_json_value(&json)
    }

    fn migrate_from_json_value(
        &mut self,
        json: &serde_json::Value,
    ) -> Result<migration::MigrationStats, String> {
        let nodes_map = json
            .get("network")
            .and_then(|n| n.get("nodes"))
            .and_then(|n| n.as_object())
            .ok_or_else(|| "V1 JSON missing network.nodes".to_string())?;

        let mut stats = migration::MigrationStats::default();
        // v1 hex → v2 Seed (for bond migration)
        let mut node_map: std::collections::HashMap<String, Seed> =
            std::collections::HashMap::new();

        // ── Stage 1 & 2: Import and type nodes ───────────────────────────────
        for (v1_hex, node_val) in nodes_map {
            let active = node_val["active"].as_bool().unwrap_or(false);
            if !active {
                continue;
            }

            let label = node_val["label"].as_str().unwrap_or("").trim();
            if label.is_empty() {
                continue;
            }

            // Try heuristic extraction on the label text.
            let ctx = temporal::ExtractionContext::new(label);
            let extracted = self.temporal.extract(&ctx);

            // Pick the first typed KU; fall back to Raw.
            let (ku, was_typed) = extracted
                .units
                .into_iter()
                .find(|ku| ku.kind() != graph::knowledge_unit::KuKind::Raw)
                .map(|ku| (ku, true))
                .unwrap_or_else(|| {
                    (
                        KnowledgeUnit::Raw {
                            text: label.to_string(),
                            source: Some("v1_migration".to_string()),
                        },
                        false,
                    )
                });

            let confidence = match node_val["confidence"].as_str() {
                Some("Terminal")   => Confidence::Terminal,
                Some("Verified")   => Confidence::Verified,
                Some("Contested")  => Confidence::Contested,
                _                  => Confidence::Unverified,
            };

            let seed = self.learn(ku, confidence);
            node_map.insert(v1_hex.clone(), seed);
            stats.total_v1_nodes += 1;
            if was_typed {
                stats.successfully_typed += 1;
            } else {
                stats.remained_raw += 1;
            }
        }

        // ── Stage 3: Migrate bonds ────────────────────────────────────────────
        if let Some(bonds_map) = json
            .get("network")
            .and_then(|n| n.get("bonds"))
            .and_then(|b| b.as_object())
        {
            for (_, bond_val) in bonds_map {
                let bond_active = bond_val["active"].as_bool().unwrap_or(true);
                if !bond_active {
                    continue;
                }

                let label = bond_val["label"].as_str().unwrap_or("");
                let bond_type = migration::map_v1_bond_label(label);
                let weight = bond_val["weight"].as_f64().unwrap_or(1.0) as f32;

                let src_hex = migration::extract_seed_hex_from_bond(bond_val, "source");
                let tgt_hex = migration::extract_seed_hex_from_bond(bond_val, "target");

                match (src_hex, tgt_hex) {
                    (Some(sh), Some(th)) => {
                        if let (Some(src), Some(tgt)) =
                            (node_map.get(&sh), node_map.get(&th))
                        {
                            let src = src.clone();
                            let tgt = tgt.clone();
                            let _ = self.associate(&src, &tgt, bond_type, weight.clamp(0.1, 5.0));
                            stats.bonds_migrated += 1;
                        } else {
                            stats.bonds_skipped += 1;
                        }
                    }
                    _ => {
                        stats.bonds_skipped += 1;
                    }
                }
            }
        }

        // ── Stage 4: Re-index ─────────────────────────────────────────────────
        self.network.rebuild_keyword_index();

        Ok(stats)
    }

    // ── Web research ──────────────────────────────────────────────────────────

    /// Run a full web research session for `query`.
    ///
    /// Pipeline: search (Grokopedia → DDG Lite fallback) → fetch pages →
    /// extract text → ingest via `learn_text()`.
    ///
    /// Returns a ResearchReport describing what was found.
    pub fn web_research(&mut self, query: &str) -> web_research::ResearchReport {
        self.web_idle.record_interaction(); // research counts as activity
        let researcher = web_research::WebResearcher::new();
        let report = researcher.research(query);
        // Ingest all extracted text blocks.
        for text in report.extracted_texts() {
            self.learn_text(text, graph::node::Confidence::Inferred);
        }
        report
    }

    /// Check whether idle auto-research should fire; if so, run it.
    ///
    /// Returns the query that was researched (if any) for logging purposes.
    /// Intended to be called from an idle/tick loop in the host application.
    pub fn web_idle_check(&mut self) -> Option<String> {
        if !self.flags.idle_research {
            return None;
        }
        if !self.flags.web_research {
            return None;
        }
        if !self.web_idle.should_research() {
            return None;
        }
        // Pick the highest-priority unresolved wonder item as the query.
        let query = self
            .curiosity
            .wonder_queue
            .pending()
            .first()
            .map(|w| w.question.clone())?;

        eprintln!("[idle] auto-research: {:?}", query);
        self.web_research(&query);
        self.web_idle.record_research();

        // Mark the wonder item as resolved.
        let id = self
            .curiosity
            .wonder_queue
            .pending()
            .first()
            .map(|w| w.id.clone());
        if let Some(id) = id {
            self.curiosity.wonder_queue.resolve(&id);
        }

        Some(query)
    }

    // ── Amygdala helpers ──────────────────────────────────────────────────────

    pub fn implant_scar(&mut self, label: &str, reason: &str, embedding: Vec<f32>) {
        let seed = df_core::Seed::generate();
        self.amygdala.record_scar(seed, label, reason, Some(embedding));
    }

    // ── Introspection ─────────────────────────────────────────────────────────

    pub fn stats(&self) -> graph::network::NetworkStats {
        self.network.stats()
    }

    /// Apply node demotions: nodes that consistently appear in low-quality
    /// recalls (≥5 times with relevance < 0.2, never with relevance > 0.6)
    /// are demoted from `Unverified` → `Contested`.
    ///
    /// Returns the number of nodes actually demoted.
    pub fn apply_demotions(&mut self) -> usize {
        let candidates = self.self_improvement.demotion_candidates();
        let mut demoted = 0;
        for seed_hex in &candidates {
            if let Ok(seed) = Seed::from_hex(seed_hex) {
                if let Some(node) = self.network.get_node_mut(&seed) {
                    if node.confidence == Confidence::Unverified {
                        node.confidence = Confidence::Contested;
                        demoted += 1;
                    }
                }
            }
        }
        demoted
    }

    pub fn terminal_bond(&self) -> &TerminalBond {
        &self.terminal_bond
    }

    pub fn health(&mut self) -> hypothalamus::HealthReport {
        let stats = self.network.stats();
        let scars = self.amygdala.scars.len();
        let pending = self.hippocampus.consolidation_queue.pending.len();
        self.hypothalamus
            .snapshot_health(stats.total_nodes, stats.total_bonds, scars, pending)
            .clone()
    }
}

impl Default for Brain {
    fn default() -> Self {
        Brain::new()
    }
}

// ── Auto-bonding helpers ───────────────────────────────────────────────────────

/// Run OCR on an image file using the Windows.Media.Ocr API (Win10/11 built-in).
///
/// No external dependencies — Tesseract, Python, or any third-party tool.
/// Falls back gracefully to `None` if OCR fails for any reason so that
/// `learn_image` never crashes.
#[cfg(windows)]
fn run_ocr(image_path: &str) -> Option<String> {
    use windows::{
        core::HSTRING,
        Graphics::Imaging::BitmapDecoder,
        Media::Ocr::OcrEngine,
        Storage::{FileAccessMode, StorageFile},
    };

    // Windows Storage APIs require an absolute path without the \\?\ UNC prefix.
    let abs = std::fs::canonicalize(image_path).ok()?;
    let path_lossy = abs.to_string_lossy();
    let clean_path = path_lossy.strip_prefix(r"\\?\").unwrap_or(&path_lossy);

    let result: Option<String> = (|| {
        let hpath = HSTRING::from(clean_path);

        // Load file → open stream → decode bitmap
        let file = StorageFile::GetFileFromPathAsync(&hpath).ok()?.get().ok()?;
        let stream = file.OpenAsync(FileAccessMode::Read).ok()?.get().ok()?;
        let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.get().ok()?;
        let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.get().ok()?;

        // Create OCR engine from the user's profile language (falls back to English)
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;

        // Run recognition
        let ocr_result = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;
        let lines = ocr_result.Lines().ok()?;
        let count = lines.Size().ok()?;

        let mut parts: Vec<String> = Vec::with_capacity(count as usize);
        for i in 0..count {
            if let Ok(line) = lines.GetAt(i) {
                if let Ok(hstr) = line.Text() {
                    let s: String = hstr.to_string();
                    if !s.trim().is_empty() {
                        parts.push(s);
                    }
                }
            }
        }

        let text = parts.join("\n").trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    })();

    if result.is_none() {
        eprintln!("[brain] Windows OCR: no text extracted from {}", image_path);
    }
    result
}

/// OCR stub for non-Windows builds (cross-compile safety net).
#[cfg(not(windows))]
fn run_ocr(image_path: &str) -> Option<String> {
    let _ = image_path;
    None
}

fn normalize_subject(s: &str) -> String {
    let lower = s.to_lowercase();
    let stripped = lower
        .strip_prefix("a ")
        .or_else(|| lower.strip_prefix("an "))
        .or_else(|| lower.strip_prefix("the "))
        .unwrap_or(&lower);
    stripped.trim().to_string()
}

fn subjects_overlap(a: &str, b: &str) -> bool {
    if temporal::PRONOUN_SUBJECTS.contains(&a) || temporal::PRONOUN_SUBJECTS.contains(&b) {
        return false;
    }
    if a == b {
        return true;
    }
    if a.len() >= 4 && b.contains(a) {
        return true;
    }
    if b.len() >= 4 && a.contains(b) {
        return true;
    }
    false
}
