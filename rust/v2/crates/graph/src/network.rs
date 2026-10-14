// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! AssociativeNetwork — the v2 knowledge graph with typed bond traversal.
//!
//! Spreading activation in v2 is *typed*: callers can restrict traversal to
//! specific BondType subsets (e.g., only follow CAUSES + REQUIRES chains when
//! answering a causal question).
//!
//! Phase 6 additions:
//!   - `keyword_index`: inverted word→node-hex index built on ingest.
//!     `keyword_search` uses the index (O(Q×hits)) instead of a full scan
//!     (O(N)) — falls back to linear scan when the index is empty (e.g.,
//!     after loading an older brain file).
//!   - `spread_activation` early-termination: propagation stops when
//!     activation drops below `ACTIVATION_THRESHOLD`.

use df_core::{Seed, SeedGated};
use embedding::EmbeddingEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::bond::{BondType, TypedBond};
use crate::knowledge_unit::KnowledgeUnit;
use crate::node::{Confidence, KnowledgeNode};

// ── Recall results ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ActivatedNode {
    pub seed: Seed,
    pub label: String,
    pub knowledge_unit: KnowledgeUnit,
    pub confidence: Confidence,
    pub strength: f32,
    pub depth: u32,
}

#[derive(Debug, Clone)]
pub struct TraversalHop {
    pub from: Seed,
    pub bond_seed: Seed,
    pub to: Seed,
    pub bond_type: BondType,
    pub strength: f32,
    pub hop: u32,
}

#[derive(Debug, Clone, Default)]
pub struct RecallResult {
    pub activated_nodes: Vec<ActivatedNode>,
    pub traversal_path: Vec<TraversalHop>,
    pub bond_types_traversed: Vec<BondType>,
}

// ── Network stats ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub revoked_nodes: usize,
    pub total_bonds: usize,
    pub active_bonds: usize,
    pub dead_bonds: usize,
}

// ── Stop words for query scoring ─────────────────────────────────────────────

/// Common stop words that should not influence search scoring or candidate
/// gathering.  These are function words, question words, and connectors.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "what", "who",
    "how", "why", "when", "where", "which", "does", "do", "did",
    "can", "could", "would", "should", "will", "has", "have",
    "had", "been", "being", "its", "it", "this", "that", "and",
    "but", "for", "with", "from", "about", "into", "than",
    "not", "also", "more", "some", "there", "their", "they",
    "you", "your", "our", "his", "her", "she", "him", "all",
    "any", "each", "every", "both", "few", "many", "much",
    "own", "other", "such", "only", "same", "just", "very",
    "still", "may", "might", "shall", "will", "too", "then",
    "now", "here", "well", "most", "over", "after", "before",
    "between", "under", "through", "during", "without",
];

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

// ── Inverted index tokenizer ──────────────────────────────────────────────────

/// Tokenise raw text for the inverted keyword index.
/// Returns lowercase tokens of length ≥ 3.
fn index_tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_lowercase())
        .collect()
}

/// Check if `word` appears in `text` at a word boundary (not as a substring
/// of a longer word).  E.g. "light" should NOT match "sunlight" or "lighting".
fn word_boundary_match(text: &str, word: &str) -> bool {
    let mut pos = 0;
    while let Some(idx) = text[pos..].find(word) {
        let abs = pos + idx;
        let before_ok = abs == 0
            || !text.as_bytes()[abs - 1].is_ascii_alphanumeric();
        let after_ok = abs + word.len() >= text.len()
            || !text.as_bytes()[abs + word.len()].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        // Advance to the next Unicode char boundary (not just +1 byte) so that
        // multi-byte characters like em-dash (—, 3 bytes) don't cause a panic.
        pos = abs + text[abs..].chars().next().map_or(1, |c| c.len_utf8());
    }
    false
}

/// Tokenise a query with graph-aware compound detection.
///
/// Before splitting on whitespace, checks whether any 2-gram or 3-gram
/// subsequence of words matches a key in `compound_index` (built from
/// node labels during `rebuild_keyword_index`).  When a compound is found
/// it is kept as a single token; the constituent words are NOT added
/// separately.
fn compound_tokenize(query: &str, compound_index: &HashMap<String, Vec<u32>>) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let n = words.len();
    let mut tokens: Vec<String> = Vec::new();
    let mut skip_until = 0usize;

    for i in 0..n {
        if i < skip_until {
            continue;
        }
        // Try 3-gram first, then 2-gram.
        let mut matched = false;
        if i + 2 < n {
            let tri = format!("{} {} {}", words[i], words[i + 1], words[i + 2]).to_lowercase();
            if compound_index.contains_key(&tri) {
                tokens.push(tri);
                skip_until = i + 3;
                matched = true;
            }
        }
        if !matched && i + 1 < n {
            let bi = format!("{} {}", words[i], words[i + 1]).to_lowercase();
            if compound_index.contains_key(&bi) {
                tokens.push(bi);
                skip_until = i + 2;
                matched = true;
            }
        }
        if !matched {
            let w = words[i].to_lowercase();
            if w.len() >= 3 {
                tokens.push(w);
            }
        }
    }
    tokens
}

// ── AssociativeNetwork ────────────────────────────────────────────────────────

/// The typed knowledge graph.
///
/// Internal maps are keyed by seed hex strings for stable serialization.
/// `bond_index[node_hex]` → list of bond hex strings touching that node.
/// `keyword_index[word]`  → list of node hex strings containing that word.
///   (not serialised — rebuilt from node raw_text after load)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociativeNetwork {
    nodes: HashMap<String, KnowledgeNode>,
    bonds: HashMap<String, TypedBond>,
    /// Adjacency index: node hex → outbound bond hexes.
    bond_index: HashMap<String, Vec<String>>,
    /// Compact node index: sequential ID → node hex string.
    /// Enables keyword/compound indexes to store u32 IDs instead of
    /// 64-char hex Strings, saving ~2.7 GB for 1.29M nodes.
    #[serde(skip)]
    node_hex_by_idx: Vec<String>,
    /// Reverse lookup: node hex → compact u32 index.
    #[serde(skip)]
    node_idx_by_hex: HashMap<String, u32>,
    /// Inverted word index: word → compact node indices.
    /// Derived from node raw_text; skipped during serialization.
    /// Call `rebuild_keyword_index()` after loading from file.
    #[serde(skip)]
    keyword_index: HashMap<String, Vec<u32>>,
    /// Compound phrase index: "machine learning" → compact node indices.
    /// Built from 2-gram and 3-gram node labels during rebuild_keyword_index().
    /// Enables graph-aware compound tokenization in keyword_search().
    #[serde(skip)]
    compound_index: HashMap<String, Vec<u32>>,
}

impl Default for AssociativeNetwork {
    fn default() -> Self {
        AssociativeNetwork::new()
    }
}

impl AssociativeNetwork {
    pub fn new() -> Self {
        AssociativeNetwork {
            nodes: HashMap::new(),
            bonds: HashMap::new(),
            bond_index: HashMap::new(),
            node_hex_by_idx: Vec::new(),
            node_idx_by_hex: HashMap::new(),
            keyword_index: HashMap::new(),
            compound_index: HashMap::new(),
        }
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Insert a node and return its seed.
    /// Also indexes the node's raw_text into `keyword_index` using compact u32 indices.
    pub fn add_node(&mut self, node: KnowledgeNode) -> Seed {
        let seed = node.seed().clone();
        let hex = seed.to_hex();

        // Assign a compact u32 index for this node.
        let idx = self.node_hex_by_idx.len() as u32;
        self.node_hex_by_idx.push(hex.clone());
        self.node_idx_by_hex.insert(hex.clone(), idx);

        // Build inverted index entry for every word in raw_text.
        let raw = node.knowledge_unit.raw_text().to_string();
        for word in index_tokenize(&raw) {
            self.keyword_index
                .entry(word)
                .or_default()
                .push(idx);
        }

        // Build compound index entries for 2-gram and 3-gram label prefixes.
        let words: Vec<&str> = raw.split_whitespace().collect();
        if words.len() >= 2 {
            let bigram = format!("{} {}", words[0], words[1]).to_lowercase();
            self.compound_index.entry(bigram).or_default().push(idx);
        }
        if words.len() >= 3 {
            let trigram = format!("{} {} {}", words[0], words[1], words[2]).to_lowercase();
            self.compound_index.entry(trigram).or_default().push(idx);
        }

        self.nodes.insert(hex.clone(), node);
        self.bond_index.entry(hex).or_default();
        seed
    }

    /// Insert a bond and update the adjacency index.  Returns the bond's seed.
    pub fn add_bond(&mut self, bond: TypedBond) -> Seed {
        let seed = bond.seed.clone();
        let src_hex = bond.source_seed.to_hex();
        let bond_hex = seed.to_hex();

        self.bond_index.entry(src_hex).or_default().push(bond_hex.clone());

        // For symmetric bonds, also index the reverse direction.
        if bond.bond_type.is_symmetric() {
            let tgt_hex = bond.target_seed.to_hex();
            self.bond_index.entry(tgt_hex).or_default().push(bond_hex.clone());
        }

        self.bonds.insert(bond_hex, bond);
        seed
    }

    // ── Retrieval ─────────────────────────────────────────────────────────────

    pub fn get_node(&self, seed: &Seed) -> Option<&KnowledgeNode> {
        self.nodes.get(&seed.to_hex())
    }

    pub fn get_node_mut(&mut self, seed: &Seed) -> Option<&mut KnowledgeNode> {
        self.nodes.get_mut(&seed.to_hex())
    }

    pub fn get_bond(&self, seed: &Seed) -> Option<&TypedBond> {
        self.bonds.get(&seed.to_hex())
    }

    /// Keyword search — fast path uses the inverted index when available;
    /// falls back to a linear scan for backwards compatibility (e.g., after
    /// loading a brain file saved before Phase 6).
    ///
    /// For multi-word queries the index is used to gather candidates and then
    /// the exact phrase is verified against raw_text — preserving the same
    /// precision as the original contains() check while avoiding a full scan.
    /// `depth` is reserved for future BFS expansion; currently unused.
    pub fn keyword_search(&self, query: &str, depth: usize, top_k: usize) -> RecallResult {
        let _ = depth; // reserved — future: expand results via BFS up to `depth` hops
        let q = query.to_lowercase();
        // Use compound-aware tokenization when the compound index is populated.
        let query_words = if self.compound_index.is_empty() {
            index_tokenize(&q)
        } else {
            compound_tokenize(&q, &self.compound_index)
        };

        let activated_nodes: Vec<ActivatedNode> = if !self.keyword_index.is_empty()
            && !query_words.is_empty()
        {
            // ── Fast path: inverted index ─────────────────────────────────────
            // Collect the union of node hexes that match any content word (skip stop words).
            // Uses compact u32 indices internally, resolved to hex for node lookup.
            let mut candidate_indices: std::collections::HashSet<u32> =
                std::collections::HashSet::new();
            for word in &query_words {
                // Skip stop words in candidate gathering — "what", "does", etc.
                // would match millions of nodes and waste scoring time.
                if is_stop_word(word.as_str()) {
                    continue;
                }
                if let Some(indices) = self.keyword_index.get(word.as_str()) {
                    candidate_indices.extend(indices.iter().copied());
                }
            }
            // Resolve u32 indices to hex strings for node lookup.
            let candidates: std::collections::HashSet<String> = candidate_indices
                .into_iter()
                .filter_map(|idx| self.node_hex_by_idx.get(idx as usize).cloned())
                .collect();

            // If index lookup found nothing (all words < 3 chars or absent),
            // fall through to the slow path.
            if candidates.is_empty() {
                self.nodes
                    .values()
                    .filter(|n| {
                        df_core::SeedGated::is_active(*n)
                            && n.knowledge_unit.raw_text().to_lowercase().contains(&q)
                    })
                    .take(top_k)
                    .map(|n| ActivatedNode {
                        seed:           n.seed().clone(),
                        label:          n.label(),
                        knowledge_unit: n.knowledge_unit.clone(),
                        confidence:     n.confidence,
                        strength:       n.confidence.weight(),
                        depth:          0,
                    })
                    .collect()
            } else {
                // Score candidates with 4-tier ranking:
                //   Tier 1 (2000+): Content-word phrase match (e.g. "big bang" in node text)
                //   Tier 2 (1000+): All content words match individually with word boundaries
                //   Tier 3 (100+):  Multiple content words match
                //   Tier 4 (1):     Single word match — fallback only
                //
                // Content words = query words minus stop words. This prevents
                // "What is the Big Bang theory?" from matching on "is" or "the".
                let content_words: Vec<&str> = query_words
                    .iter()
                    .filter(|w| !is_stop_word(w.as_str()))
                    .map(|s| s.as_str())
                    .collect();

                // Build bigram phrases from adjacent content words in original query order.
                // E.g. "big bang theory" → ["big bang", "bang theory"]
                let query_bigrams: Vec<String> = if content_words.len() >= 2 {
                    content_words.windows(2)
                        .map(|pair| format!("{} {}", pair[0], pair[1]))
                        .collect()
                } else {
                    vec![]
                };

                let mut scored: Vec<(String, usize)> = candidates
                    .into_iter()
                    .filter_map(|hex| {
                        let node = self.nodes.get(&hex)?;
                        if !df_core::SeedGated::is_active(node) {
                            return None;
                        }
                        let raw_lower = node.knowledge_unit.raw_text().to_lowercase();

                        // Single content-word query: index hit is sufficient.
                        if content_words.len() <= 1 {
                            return Some((hex, 100));
                        }

                        // Tier 1: Check if any query bigram appears as a phrase in node text.
                        // "black hole" as adjacent words is a MUCH stronger signal than
                        // "black" and "hole" separately.
                        let bigram_hits = query_bigrams.iter()
                            .filter(|bg| raw_lower.contains(bg.as_str()))
                            .count();
                        if bigram_hits > 0 {
                            // Count individual content word hits too for sub-ranking.
                            let word_hits = content_words.iter()
                                .filter(|w| word_boundary_match(&raw_lower, w))
                                .count();
                            return Some((hex, 2000 + bigram_hits * 100 + word_hits));
                        }

                        // Tier 2-4: count individual content word hits with word boundaries.
                        let hits = content_words
                            .iter()
                            .filter(|w| word_boundary_match(&raw_lower, w))
                            .count();

                        if hits == content_words.len() && content_words.len() >= 2 {
                            Some((hex, 1000 + hits))
                        } else if hits >= 2 {
                            Some((hex, 100 + hits))
                        } else if hits == 1 {
                            Some((hex, 1))
                        } else {
                            None
                        }
                    })
                    .collect();

                scored.sort_by(|a, b| b.1.cmp(&a.1));

                // For multi-word queries: if we have ANY multi-word matches,
                // drop all single-word-only matches (score == 1) to prevent
                // "Machine Gun Kelly" drowning out actual ML content.
                if query_words.len() >= 2 {
                    let has_good_matches = scored.iter().any(|(_, s)| *s > 1);
                    if has_good_matches {
                        scored.retain(|(_, s)| *s > 1);
                    }
                }

                // Compute max score for normalisation.
                let max_score = scored.iter().map(|(_, s)| *s).max().unwrap_or(1).max(1);

                scored
                    .into_iter()
                    .take(top_k)
                    .filter_map(|(hex, score)| {
                        let n = self.nodes.get(&hex)?;
                        // Scale initial strength by match quality:
                        // full-phrase match → full confidence weight,
                        // partial matches → proportionally less.
                        let relevance = score as f32 / max_score as f32;
                        Some(ActivatedNode {
                            seed:           n.seed().clone(),
                            label:          n.label(),
                            knowledge_unit: n.knowledge_unit.clone(),
                            confidence:     n.confidence,
                            strength:       n.confidence.weight() * relevance,
                            depth:          0,
                        })
                    })
                    .collect()
            }
        } else {
            // ── Slow path: linear scan ────────────────────────────────────────
            self.nodes
                .values()
                .filter(|n| {
                    df_core::SeedGated::is_active(*n)
                        && n.knowledge_unit.raw_text().to_lowercase().contains(&q)
                })
                .take(top_k)
                .map(|n| ActivatedNode {
                    seed:           n.seed().clone(),
                    label:          n.label(),
                    knowledge_unit: n.knowledge_unit.clone(),
                    confidence:     n.confidence,
                    strength:       n.confidence.weight(),
                    depth:          0,
                })
                .collect()
        };

        RecallResult {
            activated_nodes,
            traversal_path: vec![],
            bond_types_traversed: vec![],
        }
    }

    /// Semantic search — uses TF-IDF embeddings to find nodes by meaning overlap.
    ///
    /// Strategy: use the keyword_index to gather a broad candidate pool (any
    /// node sharing at least one significant token with the query), then score
    /// each candidate by cosine similarity of their TF-IDF embedding against
    /// the query embedding.  Embeddings are computed on-the-fly (StubEngine is
    /// just hash projections — microseconds per node) so this adds zero memory.
    ///
    /// Returns a `RecallResult` with strength set to cosine similarity.
    pub fn semantic_search(&self, query: &str, top_k: usize) -> RecallResult {
        let engine = embedding::StubEngine::new(384);
        let query_emb = match engine.embed(query) {
            Ok(v) => v,
            Err(_) => return RecallResult::default(),
        };

        // Broad candidate gathering: tokenize query using embedding tokenizer
        // (which includes bigrams) and pull ALL matching compact node indices
        // from the keyword_index.  This is intentionally wider than keyword_search —
        // a node matching even one token is a candidate for semantic scoring.
        let tokens = embedding::embed_tokenize(query);
        let mut candidate_indices: std::collections::HashSet<u32> =
            std::collections::HashSet::new();
        for token in &tokens {
            if let Some(indices) = self.keyword_index.get(token.as_str()) {
                candidate_indices.extend(indices.iter().copied());
            }
        }
        // Also look up the raw lowercased words (no bigrams) in case
        // embed_tokenize filtered something that index_tokenize kept.
        for word in query.split_whitespace() {
            let w = word.to_lowercase();
            if w.len() >= 3 {
                if let Some(indices) = self.keyword_index.get(w.as_str()) {
                    candidate_indices.extend(indices.iter().copied());
                }
            }
        }
        // Resolve u32 indices to hex strings for node lookup.
        let candidate_hexes: std::collections::HashSet<String> = candidate_indices
            .into_iter()
            .filter_map(|idx| self.node_hex_by_idx.get(idx as usize).cloned())
            .collect();

        if candidate_hexes.is_empty() {
            return RecallResult::default();
        }

        // Score each candidate by cosine similarity of their TF-IDF embedding.
        let mut scored: Vec<(String, f32)> = candidate_hexes
            .into_iter()
            .filter_map(|hex| {
                let node = self.nodes.get(&hex)?;
                if !df_core::SeedGated::is_active(node) {
                    return None;
                }
                let raw = node.knowledge_unit.raw_text();
                let node_emb = engine.embed(raw).ok()?;
                let sim = embedding::cosine_similarity(&query_emb, &node_emb);
                if sim > 0.05 {
                    Some((hex, sim))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        let activated_nodes: Vec<ActivatedNode> = scored
            .into_iter()
            .filter_map(|(hex, sim)| {
                let n = self.nodes.get(&hex)?;
                Some(ActivatedNode {
                    seed:           n.seed().clone(),
                    label:          n.label(),
                    knowledge_unit: n.knowledge_unit.clone(),
                    confidence:     n.confidence,
                    strength:       n.confidence.weight() * sim,
                    depth:          0,
                })
            })
            .collect();

        RecallResult {
            activated_nodes,
            traversal_path: vec![],
            bond_types_traversed: vec![],
        }
    }

    /// Rebuild the keyword index from all active nodes' raw_text.
    /// Also rebuilds the compact node index, compound_index, and graph-aware tokenization.
    /// Call this after loading a brain file (the indexes are not serialised).
    pub fn rebuild_keyword_index(&mut self) {
        self.keyword_index.clear();
        self.compound_index.clear();
        self.node_hex_by_idx.clear();
        self.node_idx_by_hex.clear();

        // First pass: assign compact u32 indices to all active nodes.
        let entries: Vec<(u32, String, String)> = self
            .nodes
            .iter()
            .filter(|(_, n)| df_core::SeedGated::is_active(*n))
            .enumerate()
            .map(|(i, (hex, n))| (i as u32, hex.clone(), n.knowledge_unit.raw_text().to_string()))
            .collect();

        self.node_hex_by_idx.reserve(entries.len());
        self.node_idx_by_hex.reserve(entries.len());

        for &(idx, ref hex, _) in &entries {
            self.node_hex_by_idx.push(hex.clone());
            self.node_idx_by_hex.insert(hex.clone(), idx);
        }

        // Second pass: build keyword and compound indexes using compact u32 indices.
        for &(idx, _, ref raw_text) in &entries {
            for word in index_tokenize(raw_text) {
                self.keyword_index.entry(word).or_default().push(idx);
            }

            // Build compound index: record 2-gram and 3-gram node label prefixes.
            let words: Vec<&str> = raw_text.split_whitespace().collect();
            if words.len() >= 2 {
                let bigram = format!("{} {}", words[0], words[1]).to_lowercase();
                self.compound_index.entry(bigram).or_default().push(idx);
            }
            if words.len() >= 3 {
                let trigram = format!("{} {} {}", words[0], words[1], words[2]).to_lowercase();
                self.compound_index.entry(trigram).or_default().push(idx);
            }
        }
    }

    /// Returns the number of compound phrases in the compound index.
    pub fn compound_index_size(&self) -> usize {
        self.compound_index.len()
    }

    /// Returns the number of unique words in the keyword index.
    pub fn keyword_index_size(&self) -> usize {
        self.keyword_index.len()
    }

    /// Typed spreading activation from a set of initial (seed, strength) pairs.
    ///
    /// `bond_filter` — if non-empty, only traverse bonds of the listed types.
    /// `depth`       — maximum hops from the initial seeds.
    /// `top_k`       — maximum nodes to return.
    pub fn typed_recall(
        &mut self,
        initial: &[(Seed, f32)],
        bond_filter: &[BondType],
        depth: usize,
        top_k: usize,
    ) -> RecallResult {
        const DECAY: f32 = 0.7;
        const ACTIVATION_CEILING: f32 = 200.0;
        const ACTIVATION_THRESHOLD: f32 = 0.001;
        const HEBBIAN_GAIN: f32 = 0.05;
        // ── Degree normalization: dampen activation flowing into high-degree
        //    hub nodes so they don't drown out precise keyword matches.
        //    Divisor = 1 + ln(bond_count).  A node with 1 bond → divisor 1.0,
        //    50 bonds → ~4.9, 1400 bonds → ~8.2.
        const DEGREE_NORM: bool = true;

        let mut activation: HashMap<String, f32> = HashMap::new();
        let mut hops: HashMap<String, u32> = HashMap::new();
        let mut path: Vec<TraversalHop> = Vec::new();
        let mut bond_types_seen: std::collections::HashSet<BondType> =
            std::collections::HashSet::new();

        // Track which nodes are direct keyword seeds (depth-0) so we can
        // give them a priority bonus in the final ranking.
        let seed_set: std::collections::HashSet<String> =
            initial.iter().map(|(s, _)| s.to_hex()).collect();

        // Seed the frontier
        let mut frontier: Vec<(String, f32, u32)> = initial
            .iter()
            .map(|(s, w)| (s.to_hex(), w.min(ACTIVATION_CEILING), 0))
            .collect();

        for (hex, w, _) in &frontier {
            activation.insert(hex.clone(), *w);
            hops.insert(hex.clone(), 0);
        }

        // Track nodes already enqueued across all depths to prevent exponential
        // frontier growth when hub nodes are reachable via multiple paths.
        let mut visited: std::collections::HashSet<String> =
            initial.iter().map(|(s, _)| s.to_hex()).collect();

        // Best-first expansion
        for current_depth in 0..depth {
            let mut next_frontier: Vec<(String, f32, u32)> = Vec::new();

            for (node_hex, node_strength, _) in &frontier {
                let bonds = match self.bond_index.get(node_hex) {
                    Some(b) => b.clone(),
                    None => continue,
                };

                for bond_hex in bonds {
                    let bond = match self.bonds.get_mut(&bond_hex) {
                        Some(b) if !b.is_dead() => b,
                        _ => continue,
                    };

                    // Apply bond type filter
                    if !bond_filter.is_empty() && !bond_filter.contains(&bond.bond_type) {
                        continue;
                    }

                    let target_hex = bond.target_seed.to_hex();

                    // Skip nodes already enqueued — prevents exponential frontier
                    // growth when multiple paths reach the same hub node.
                    if visited.contains(&target_hex) {
                        continue;
                    }

                    let target_active = self
                        .nodes
                        .get(&target_hex)
                        .map(|n| df_core::SeedGated::is_active(n))
                        .unwrap_or(false);

                    if !target_active {
                        continue;
                    }

                    // Strength decays with each hop, weighted by bond weight
                    let mut propagated = node_strength
                        * DECAY.powi(current_depth as i32 + 1)
                        * bond.weight
                        * bond.bond_type.default_weight();

                    // ── Degree normalization ──────────────────────────────────
                    // Divide incoming activation by log(degree) of the target
                    // so high-connectivity hub nodes don't accumulate unfairly.
                    if DEGREE_NORM {
                        let target_degree = self
                            .bond_index
                            .get(&target_hex)
                            .map(|b| b.len())
                            .unwrap_or(1)
                            .max(1) as f32;
                        let norm = 1.0 + target_degree.ln();
                        propagated /= norm;
                    }

                    // Early termination: skip if propagation is negligible.
                    if propagated < ACTIVATION_THRESHOLD {
                        continue;
                    }

                    let current = activation.entry(target_hex.clone()).or_insert(0.0);
                    if propagated > *current * 0.1 {
                        *current += propagated;
                        bond_types_seen.insert(bond.bond_type);

                        // Hebbian strengthening
                        bond.strengthen(HEBBIAN_GAIN);

                        if let Ok(src) = Seed::from_hex(node_hex) {
                            path.push(TraversalHop {
                                from: src,
                                bond_seed: bond.seed.clone(),
                                to: bond.target_seed.clone(),
                                bond_type: bond.bond_type,
                                strength: propagated,
                                hop: current_depth as u32 + 1,
                            });
                        }

                        visited.insert(target_hex.clone());
                        next_frontier.push((
                            target_hex.clone(),
                            *current,
                            current_depth as u32 + 1,
                        ));
                        hops.insert(target_hex, current_depth as u32 + 1);
                    }
                }
            }

            if next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        // Collect and rank results
        let mut results: Vec<(&KnowledgeNode, f32, u32)> = self
            .nodes
            .iter()
            .filter_map(|(hex, node)| {
                if !df_core::SeedGated::is_active(node) {
                    return None;
                }
                let mut strength = *activation.get(hex)?;
                let hop = *hops.get(hex).unwrap_or(&0);

                // ── Seed priority bonus ──────────────────────────────────
                // Direct keyword matches (depth-0 nodes) get a 5× boost so
                // they always outrank traversal noise from hub nodes.
                if seed_set.contains(hex) {
                    strength *= 5.0;
                }

                Some((node, strength, hop))
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let activated_nodes: Vec<ActivatedNode> = results
            .into_iter()
            .take(top_k)
            .map(|(node, strength, depth)| ActivatedNode {
                seed: node.seed().clone(),
                label: node.label(),
                knowledge_unit: node.knowledge_unit.clone(),
                confidence: node.confidence,
                strength,
                depth,
            })
            .collect();

        RecallResult {
            activated_nodes,
            traversal_path: path,
            bond_types_traversed: bond_types_seen.into_iter().collect(),
        }
    }

    /// Untyped spreading activation — follows all bond types with equal priority.
    pub fn spread_activation(
        &mut self,
        initial: &[(Seed, f32)],
        depth: usize,
        top_k: usize,
    ) -> RecallResult {
        self.typed_recall(initial, &[], depth, top_k)
    }

    // ── Stats ─────────────────────────────────────────────────────────────────

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    pub fn active_node_count(&self) -> usize {
        self.nodes.values().filter(|n| df_core::SeedGated::is_active(*n)).count()
    }

    /// All bonds as a cloned vec (for inspection / consolidation).
    pub fn all_bonds(&self) -> Vec<TypedBond> {
        self.bonds.values().cloned().collect()
    }

    /// Lazy iterator over bonds — avoids cloning all 1.4M bonds into a Vec.
    pub fn iter_bonds(&self) -> impl Iterator<Item = &TypedBond> {
        self.bonds.values()
    }

    /// Mutable access to a bond by its seed.
    pub fn get_bond_mut(&mut self, seed: &Seed) -> Option<&mut TypedBond> {
        self.bonds.get_mut(&seed.to_hex())
    }

    /// Collected vec of all active nodes (borrowed) — expensive for large graphs.
    pub fn active_nodes(&self) -> Vec<&KnowledgeNode> {
        self.nodes
            .values()
            .filter(|n| df_core::SeedGated::is_active(*n))
            .collect()
    }

    /// Lazy iterator over active nodes — avoids collecting 1.7M nodes into a Vec.
    pub fn iter_active_nodes(&self) -> impl Iterator<Item = &KnowledgeNode> {
        self.nodes
            .values()
            .filter(|n| df_core::SeedGated::is_active(*n))
    }

    pub fn stats(&self) -> NetworkStats {
        let total_nodes = self.nodes.len();
        let active_nodes = self.active_node_count();
        let total_bonds = self.bonds.len();
        let active_bonds = self.bonds.values().filter(|b| b.active).count();
        NetworkStats {
            total_nodes,
            active_nodes,
            revoked_nodes: total_nodes - active_nodes,
            total_bonds,
            active_bonds,
            dead_bonds: total_bonds - active_bonds,
        }
    }
}
